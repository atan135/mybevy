[CmdletBinding(PositionalBinding = $false)]
param(
    [string[]]$Screens = @("ui-gallery"),
    [string[]]$Devices = @("all"),
    [string]$States = "auto",
    [string]$RunId = "",
    [string]$OutputRoot = "",
    [int]$TimeoutSeconds = 600,
    [string]$RerunFromManifest = "",
    [ValidateSet("FailedOnly", "ScreenMatrix")]
    [string]$RerunMode = "FailedOnly",
    [string]$WindowProfile = "",
    [string]$WindowSize = "",
    [string]$DeviceScale = "",
    [string]$WindowScale = "",
    [string[]]$BevyArgs = @(),
    [switch]$DryRun,
    [switch]$SelfTest,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$RemainingArgs = @()
)

$ErrorActionPreference = "Stop"

$script:BasicDevices = @(
    "desktop",
    "phone-small",
    "phone-portrait",
    "phone-1080p",
    "tablet-portrait",
    "tablet-landscape"
)

$script:KnownScreens = @(
    [pscustomobject]@{ Canonical = "login"; Aliases = @("login") },
    [pscustomobject]@{ Canonical = "lobby"; Aliases = @("lobby", "game_list", "game-list", "list") },
    [pscustomobject]@{ Canonical = "audio_settings"; Aliases = @("audio_settings", "audio-settings", "audio", "settings") },
    [pscustomobject]@{ Canonical = "audio_monitor"; Aliases = @("audio_monitor", "audio-monitor", "audio_debug", "audio-debug") },
    [pscustomobject]@{ Canonical = "audio_gallery"; Aliases = @("audio_gallery", "audio-gallery") },
    [pscustomobject]@{ Canonical = "wanfa_touch_ripple"; Aliases = @("wanfa_touch_ripple", "wanfa-touch-ripple", "touch", "touch_ripple", "touch-ripple") },
    [pscustomobject]@{ Canonical = "ui_gallery"; Aliases = @("ui_gallery", "ui-gallery", "gallery") },
    [pscustomobject]@{ Canonical = "sample_scene"; Aliases = @("sample_scene", "sample-scene", "sample") },
    [pscustomobject]@{ Canonical = "robot_sync_scene"; Aliases = @("robot_sync_scene", "robot-sync-scene", "robot") },
    [pscustomobject]@{ Canonical = "fangyuan_home"; Aliases = @("fangyuan_home", "fangyuan-home", "fangyuan") }
)

function Split-UiAuditList {
    param([object[]]$Values)

    $items = New-Object System.Collections.Generic.List[string]
    foreach ($value in $Values) {
        if ($null -eq $value) {
            continue
        }

        foreach ($part in ([string]$value -split "[,;]")) {
            $trimmed = $part.Trim()
            if ($trimmed.Length -gt 0) {
                $items.Add($trimmed)
            }
        }
    }

    return @($items.ToArray())
}

function Normalize-UiAuditToken {
    param([Parameter(Mandatory = $true)][string]$Value)

    return $Value.Trim().ToLowerInvariant().Replace("-", "_")
}

function Get-SafePathSegment {
    param([Parameter(Mandatory = $true)][string]$Value)

    $safe = ($Value.Trim().ToLowerInvariant() -replace "[^a-z0-9._-]", "_").Trim("_")
    if ([string]::IsNullOrWhiteSpace($safe)) {
        return "item"
    }
    return $safe
}

function Resolve-UiAuditScreens {
    param([object[]]$InputScreens)

    $tokens = Split-UiAuditList $InputScreens
    if ($tokens.Count -eq 0) {
        throw "At least one screen is required."
    }

    if ($tokens | Where-Object { $_.Trim().ToLowerInvariant() -in @("all", "full") }) {
        return @($script:KnownScreens | ForEach-Object { $_.Canonical })
    }

    $resolved = New-Object System.Collections.Generic.List[string]
    foreach ($token in $tokens) {
        $normalized = Normalize-UiAuditToken $token
        $screen = $script:KnownScreens | Where-Object {
            (Normalize-UiAuditToken $_.Canonical) -eq $normalized -or
                ($_.Aliases | Where-Object { (Normalize-UiAuditToken $_) -eq $normalized })
        } | Select-Object -First 1

        if ($null -eq $screen) {
            $known = ($script:KnownScreens | ForEach-Object { $_.Canonical }) -join ", "
            throw "Unknown UI audit screen '$token'. Known screens: $known"
        }

        if (-not $resolved.Contains($screen.Canonical)) {
            $resolved.Add($screen.Canonical)
        }
    }

    return @($resolved.ToArray())
}

function Resolve-UiAuditDevices {
    param([object[]]$InputDevices)

    $tokens = Split-UiAuditList $InputDevices
    if ($tokens.Count -eq 0) {
        throw "At least one device is required."
    }

    if ($tokens | Where-Object { $_.Trim().ToLowerInvariant() -in @("all", "full") }) {
        return @($script:BasicDevices)
    }

    $resolved = New-Object System.Collections.Generic.List[string]
    foreach ($token in $tokens) {
        $device = $token.Trim().ToLowerInvariant()
        if ($device -notin $script:BasicDevices) {
            throw "Unknown UI audit device '$token'. Known devices: $($script:BasicDevices -join ', ')"
        }

        if (-not $resolved.Contains($device)) {
            $resolved.Add($device)
        }
    }

    return @($resolved.ToArray())
}

function Resolve-UiAuditStates {
    param(
        [Parameter(Mandatory = $true)][string]$Screen,
        [Parameter(Mandatory = $true)][string]$StateValue
    )

    if ($StateValue.Trim().Equals("auto", [System.StringComparison]::OrdinalIgnoreCase)) {
        if ($Screen -eq "ui_gallery") {
            return "top,middle,bottom"
        }
        return "initial"
    }

    $valid = @("initial", "top", "middle", "bottom")
    $states = Split-UiAuditList @($StateValue)
    if ($states.Count -eq 0) {
        throw "At least one audit state is required when -States is not auto."
    }

    $normalized = New-Object System.Collections.Generic.List[string]
    foreach ($state in $states) {
        $name = $state.Trim().ToLowerInvariant()
        if ($name -notin $valid) {
            throw "Unknown UI audit state '$state'. Known states: $($valid -join ', ')"
        }
        $normalized.Add($name)
    }

    return ($normalized.ToArray() -join ",")
}

function New-UiAuditRunId {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $suffix = [Guid]::NewGuid().ToString("N").Substring(0, 6)
    return "$stamp-$suffix"
}

function Get-FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    return [System.IO.Path]::GetFullPath($Path)
}

function Join-FullPath {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Child
    )

    return Get-FullPath (Join-Path $Root $Child)
}

function Get-RelativePathCompat {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$TargetPath
    )

    $baseFull = Get-FullPath $BasePath
    $targetFull = Get-FullPath $TargetPath

    if ([System.IO.Path]::GetPathRoot($baseFull) -ne [System.IO.Path]::GetPathRoot($targetFull)) {
        return ($targetFull -replace "\\", "/")
    }

    if (-not $baseFull.EndsWith([System.IO.Path]::DirectorySeparatorChar)) {
        $baseFull = $baseFull + [System.IO.Path]::DirectorySeparatorChar
    }

    $baseUri = [Uri]$baseFull
    $targetUri = [Uri]$targetFull
    $relative = [Uri]::UnescapeDataString($baseUri.MakeRelativeUri($targetUri).ToString())
    return ($relative -replace "\\", "/")
}

function Resolve-ArtifactPath {
    param(
        [string]$Value,
        [Parameter(Mandatory = $true)][string]$TaskOutputDir
    )

    if ([string]::IsNullOrWhiteSpace($Value)) {
        return $null
    }

    if ([System.IO.Path]::IsPathRooted($Value)) {
        return Get-FullPath $Value
    }

    return Join-FullPath $TaskOutputDir $Value
}

function ConvertTo-RunRelativePath {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [string]$Path
    )

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return $null
    }

    return Get-RelativePathCompat $RunRoot $Path
}

function ConvertTo-CommandLineArgument {
    param([Parameter(Mandatory = $true)][string]$Value)

    if ($Value -notmatch '[\s"]') {
        return $Value
    }

    $escaped = $Value -replace '(\\*)"', '$1$1\"'
    $escaped = $escaped -replace '(\\+)$', '$1$1'
    return '"' + $escaped + '"'
}

function Set-ProcessArguments {
    param(
        [Parameter(Mandatory = $true)][System.Diagnostics.ProcessStartInfo]$ProcessStartInfo,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    $argumentListProperty = $ProcessStartInfo.PSObject.Properties["ArgumentList"]
    if ($null -ne $argumentListProperty) {
        foreach ($argument in $Arguments) {
            [void]$ProcessStartInfo.ArgumentList.Add($argument)
        }
        return
    }

    $ProcessStartInfo.Arguments = (($Arguments | ForEach-Object { ConvertTo-CommandLineArgument $_ }) -join " ")
}

function Stop-ProcessTreeCompat {
    param([Parameter(Mandatory = $true)][System.Diagnostics.Process]$Process)

    if ($Process.HasExited) {
        return
    }

    if ([System.IO.Path]::DirectorySeparatorChar -eq '\') {
        & taskkill.exe /PID $Process.Id /T /F | Out-Null
        return
    }

    try {
        $Process.Kill($true)
    } catch {
        $Process.Kill()
    }
}

function Get-WindowArgumentOverrides {
    param(
        [string]$WindowProfileValue,
        [string]$WindowSizeValue,
        [string]$DeviceScaleValue,
        [string]$WindowScaleValue,
        [string[]]$RawBevyArgs,
        [string[]]$RawRemainingArgs
    )

    $args = New-Object System.Collections.Generic.List[string]
    if (-not [string]::IsNullOrWhiteSpace($WindowProfileValue)) {
        $args.Add("--window-profile")
        $args.Add($WindowProfileValue)
    }
    if (-not [string]::IsNullOrWhiteSpace($WindowSizeValue)) {
        $args.Add("--window-size")
        $args.Add($WindowSizeValue)
    }
    if (-not [string]::IsNullOrWhiteSpace($DeviceScaleValue)) {
        $args.Add("--device-scale")
        $args.Add($DeviceScaleValue)
    }
    if (-not [string]::IsNullOrWhiteSpace($WindowScaleValue)) {
        $args.Add("--window-scale")
        $args.Add($WindowScaleValue)
    }
    foreach ($arg in $RawBevyArgs) {
        if (-not [string]::IsNullOrWhiteSpace($arg)) {
            $args.Add($arg)
        }
    }
    foreach ($arg in $RawRemainingArgs) {
        if (-not [string]::IsNullOrWhiteSpace($arg)) {
            $args.Add($arg)
        }
    }

    return @($args.ToArray())
}

function New-UiAuditTask {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$Screen,
        [Parameter(Mandatory = $true)][string]$Device,
        [Parameter(Mandatory = $true)][string]$StateValue,
        [AllowNull()][string[]]$ExtraBevyArgs
    )

    $screenSegment = Get-SafePathSegment $Screen
    $deviceSegment = Get-SafePathSegment $Device
    $outputDir = Join-FullPath $RunRoot (Join-Path "runs" (Join-Path $screenSegment $deviceSegment))
    $logPrefix = Join-FullPath $RunRoot (Join-Path "logs" "$screenSegment`__$deviceSegment")
    $statesForScreen = Resolve-UiAuditStates -Screen $Screen -StateValue $StateValue
    $extraArgs = if ($null -eq $ExtraBevyArgs) { @() } else { @($ExtraBevyArgs) }
    $bevyArgsForDevice = @("--window-profile", $Device) + $extraArgs

    return [pscustomobject]@{
        screen = $Screen
        requested_screen = $Screen
        device = $Device
        states = $statesForScreen
        output_dir = $outputDir
        stdout_log = "$logPrefix.stdout.log"
        stderr_log = "$logPrefix.stderr.log"
        cargo_args = @("run", "--") + $bevyArgsForDevice
        bevy_args = $bevyArgsForDevice
    }
}

function New-UiAuditTasks {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string[]]$ScreensToRun,
        [Parameter(Mandatory = $true)][string[]]$DevicesToRun,
        [Parameter(Mandatory = $true)][string]$StateValue,
        [AllowNull()][string[]]$ExtraBevyArgs
    )

    $tasks = New-Object System.Collections.Generic.List[object]
    foreach ($screen in $ScreensToRun) {
        foreach ($device in $DevicesToRun) {
            $tasks.Add((New-UiAuditTask -RunRoot $RunRoot -Screen $screen -Device $device -StateValue $StateValue -ExtraBevyArgs $ExtraBevyArgs))
        }
    }
    return @($tasks.ToArray())
}

function Read-JsonFile {
    param([Parameter(Mandatory = $true)][string]$Path)

    return (Get-Content -Raw -Path $Path | ConvertFrom-Json)
}

function Get-FailedTaskSeedsFromManifest {
    param(
        [Parameter(Mandatory = $true)][string]$ManifestPath,
        [Parameter(Mandatory = $true)][string]$Mode,
        [Parameter(Mandatory = $true)][string[]]$MatrixDevices
    )

    if (-not (Test-Path $ManifestPath)) {
        throw "Rerun manifest not found: $ManifestPath"
    }

    $manifest = Read-JsonFile $ManifestPath
    if ($null -eq $manifest.tasks) {
        throw "Rerun manifest does not contain runner tasks: $ManifestPath"
    }

    $failed = @($manifest.tasks | Where-Object { $_.status -ne "passed" -and $_.status -ne "planned" })
    if ($failed.Count -eq 0) {
        return @()
    }

    $seeds = New-Object System.Collections.Generic.List[object]
    if ($Mode -eq "FailedOnly") {
        foreach ($task in $failed) {
            $screen = [string]$task.screen
            $device = [string]$task.device
            if ([string]::IsNullOrWhiteSpace($screen) -or [string]::IsNullOrWhiteSpace($device)) {
                throw "Failed task in rerun manifest is missing screen or device."
            }
            $seeds.Add([pscustomobject]@{ screen = $screen; device = $device })
        }
    } else {
        $screens = @($failed | ForEach-Object { [string]$_.screen } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Unique)
        foreach ($screen in $screens) {
            foreach ($device in $MatrixDevices) {
                $seeds.Add([pscustomobject]@{ screen = $screen; device = $device })
            }
        }
    }

    return @($seeds.ToArray())
}

function New-UiAuditTasksFromSeeds {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][object[]]$Seeds,
        [Parameter(Mandatory = $true)][string]$StateValue,
        [AllowNull()][string[]]$ExtraBevyArgs
    )

    $tasks = New-Object System.Collections.Generic.List[object]
    foreach ($seed in $Seeds) {
        $tasks.Add((New-UiAuditTask -RunRoot $RunRoot -Screen ([string]$seed.screen) -Device ([string]$seed.device) -StateValue $StateValue -ExtraBevyArgs $ExtraBevyArgs))
    }
    return @($tasks.ToArray())
}

function Invoke-UiAuditCargoRun {
    param(
        [Parameter(Mandatory = $true)][object]$Task,
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds
    )

    New-Item -ItemType Directory -Force -Path $Task.output_dir | Out-Null
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Task.stdout_log) | Out-Null

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = "cargo"
    $psi.WorkingDirectory = $ProjectRoot
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    Set-ProcessArguments -ProcessStartInfo $psi -Arguments ([string[]]$Task.cargo_args)

    $psi.Environment["MYBEVY_UI_AUDIT"] = "1"
    $psi.Environment["MYBEVY_UI_AUDIT_SCREEN"] = [string]$Task.requested_screen
    $psi.Environment["MYBEVY_UI_AUDIT_OUTPUT"] = [string]$Task.output_dir
    $psi.Environment["MYBEVY_UI_AUDIT_STATES"] = [string]$Task.states
    $psi.Environment["MYBEVY_UI_AUDIT_EXIT_ON_FINISH"] = "1"

    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $psi

    try {
        try {
            [void]$process.Start()
        } catch {
            Set-Content -Path $Task.stdout_log -Value "" -Encoding UTF8
            Set-Content -Path $Task.stderr_log -Value $_.Exception.Message -Encoding UTF8
            return [pscustomobject]@{
                started = $false
                launch_error = $_.Exception.Message
                timed_out = $false
                exit_code = $null
            }
        }

        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()

        $waitMs = [Math]::Max(1, $TimeoutSeconds) * 1000
        $completed = $process.WaitForExit($waitMs)
        if (-not $completed) {
            try {
                Stop-ProcessTreeCompat -Process $process
            } catch {
                Set-Content -Path $Task.stderr_log -Value "Failed to kill timed-out process: $($_.Exception.Message)" -Encoding UTF8
            }
            [void]$process.WaitForExit(10000)
            Set-Content -Path $Task.stdout_log -Value $stdoutTask.GetAwaiter().GetResult() -Encoding UTF8
            Set-Content -Path $Task.stderr_log -Value $stderrTask.GetAwaiter().GetResult() -Encoding UTF8
            return [pscustomobject]@{
                started = $true
                launch_error = $null
                timed_out = $true
                exit_code = $null
            }
        }

        $process.WaitForExit()
        Set-Content -Path $Task.stdout_log -Value $stdoutTask.GetAwaiter().GetResult() -Encoding UTF8
        Set-Content -Path $Task.stderr_log -Value $stderrTask.GetAwaiter().GetResult() -Encoding UTF8
        return [pscustomobject]@{
            started = $true
            launch_error = $null
            timed_out = $false
            exit_code = $process.ExitCode
        }
    } finally {
        $process.Dispose()
    }
}

function Convert-ChildEntriesToCaptures {
    param(
        [Parameter(Mandatory = $true)]$ChildManifest,
        [Parameter(Mandatory = $true)][object]$Task,
        [Parameter(Mandatory = $true)][string]$RunRoot
    )

    $captures = New-Object System.Collections.Generic.List[object]
    if ($null -eq $ChildManifest.entries) {
        return @($captures.ToArray())
    }

    foreach ($entry in @($ChildManifest.entries)) {
        $screenshotPath = Resolve-ArtifactPath -Value ([string]$entry.screenshot_path) -TaskOutputDir ([string]$Task.output_dir)
        $metadataPath = Resolve-ArtifactPath -Value ([string]$entry.metadata_path) -TaskOutputDir ([string]$Task.output_dir)
        $screenshotExists = ($null -ne $screenshotPath -and (Test-Path $screenshotPath))
        $metadataExists = ($null -ne $metadataPath -and (Test-Path $metadataPath))

        $captures.Add([pscustomobject]@{
            screen = [string]$entry.screen
            requested_screen = [string]$entry.requested_screen
            device = [string]$Task.device
            rendered_device = [string]$entry.device
            state = [string]$entry.state
            status = [string]$entry.status
            failure = $entry.failure
            detail = $entry.detail
            screenshot = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $screenshotPath
            metadata = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $metadataPath
            screenshot_exists = $screenshotExists
            metadata_exists = $metadataExists
            scroll_target_id = $entry.scroll_target_id
            scroll_position = $entry.scroll_position
        })
    }

    return @($captures.ToArray())
}

function Resolve-UiAuditTaskResult {
    param(
        [Parameter(Mandatory = $true)][object]$Task,
        [Parameter(Mandatory = $true)][object]$LaunchResult,
        [Parameter(Mandatory = $true)][string]$RunRoot
    )

    $childManifestPath = Join-FullPath $Task.output_dir "manifest.json"
    $childReportPath = Join-FullPath $Task.output_dir "report.md"
    $base = [ordered]@{
        screen = [string]$Task.screen
        requested_screen = [string]$Task.requested_screen
        device = [string]$Task.device
        states = [string]$Task.states
        status = "failed"
        failure_type = $null
        detail = $null
        exit_code = $LaunchResult.exit_code
        timed_out = [bool]$LaunchResult.timed_out
        output_dir = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path ([string]$Task.output_dir)
        stdout = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path ([string]$Task.stdout_log)
        stderr = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path ([string]$Task.stderr_log)
        child_manifest = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $childManifestPath
        child_report = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $childReportPath
        cargo_args = @($Task.cargo_args)
        bevy_args = @($Task.bevy_args)
        captures = @()
    }

    if (-not [bool]$LaunchResult.started) {
        $base.failure_type = "launch_failed"
        $base.detail = [string]$LaunchResult.launch_error
        return [pscustomobject]$base
    }

    if ([bool]$LaunchResult.timed_out) {
        $base.failure_type = "timeout"
        $base.detail = "cargo run exceeded timeout of $TimeoutSeconds seconds"
        return [pscustomobject]$base
    }

    if (-not (Test-Path $childManifestPath)) {
        if ($null -ne $LaunchResult.exit_code -and [int]$LaunchResult.exit_code -ne 0) {
            $base.failure_type = "launch_failed"
            $base.detail = "cargo run exited with code $($LaunchResult.exit_code) before writing child manifest"
        } else {
            $base.failure_type = "manifest_missing"
            $base.detail = "child manifest was not written"
        }
        return [pscustomobject]$base
    }

    try {
        $childManifest = Read-JsonFile $childManifestPath
    } catch {
        $base.failure_type = "manifest_invalid"
        $base.detail = $_.Exception.Message
        return [pscustomobject]$base
    }

    $captures = @(Convert-ChildEntriesToCaptures -ChildManifest $childManifest -Task $Task -RunRoot $RunRoot)
    $base.captures = $captures
    if ($captures.Count -eq 0) {
        $base.failure_type = "output_missing"
        $base.detail = "child manifest has no capture entries"
        return [pscustomobject]$base
    }

    $failedCaptures = @($captures | Where-Object { $_.status -ne "passed" })
    if ($failedCaptures.Count -gt 0) {
        $base.failure_type = "audit_failed"
        $base.detail = (($failedCaptures | ForEach-Object {
            $failure = if ($_.failure) { [string]$_.failure } else { "unknown" }
            if ($_.detail) {
                "$($_.state): $failure ($($_.detail))"
            } else {
                "$($_.state): $failure"
            }
        }) -join "; ")
        return [pscustomobject]$base
    }

    $missingOutputs = @($captures | Where-Object { -not $_.screenshot_exists -or -not $_.metadata_exists })
    if ($missingOutputs.Count -gt 0) {
        $base.failure_type = "output_missing"
        $base.detail = (($missingOutputs | ForEach-Object {
            $missing = New-Object System.Collections.Generic.List[string]
            if (-not $_.screenshot_exists) { $missing.Add("screenshot") }
            if (-not $_.metadata_exists) { $missing.Add("metadata") }
            "$($_.state): $($missing.ToArray() -join '+')"
        }) -join "; ")
        return [pscustomobject]$base
    }

    if ($null -ne $LaunchResult.exit_code -and [int]$LaunchResult.exit_code -ne 0) {
        $base.failure_type = "process_failed"
        $base.detail = "cargo run exited with code $($LaunchResult.exit_code)"
        return [pscustomobject]$base
    }

    $base.status = "passed"
    return [pscustomobject]$base
}

function New-PlannedTaskResult {
    param(
        [Parameter(Mandatory = $true)][object]$Task,
        [Parameter(Mandatory = $true)][string]$RunRoot
    )

    return [pscustomobject]@{
        screen = [string]$Task.screen
        requested_screen = [string]$Task.requested_screen
        device = [string]$Task.device
        states = [string]$Task.states
        status = "planned"
        failure_type = $null
        detail = "dry run; cargo was not started"
        exit_code = $null
        timed_out = $false
        output_dir = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path ([string]$Task.output_dir)
        stdout = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path ([string]$Task.stdout_log)
        stderr = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path ([string]$Task.stderr_log)
        child_manifest = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path (Join-FullPath $Task.output_dir "manifest.json")
        child_report = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path (Join-FullPath $Task.output_dir "report.md")
        cargo_args = @($Task.cargo_args)
        bevy_args = @($Task.bevy_args)
        captures = @()
    }
}

function Write-UiAuditRunnerOutputs {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$RunIdValue,
        [Parameter(Mandatory = $true)][object[]]$Results,
        [Parameter(Mandatory = $true)][string[]]$ScreensValue,
        [Parameter(Mandatory = $true)][string[]]$DevicesValue,
        [Parameter(Mandatory = $true)][bool]$IsDryRun,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$RerunSource
    )

    New-Item -ItemType Directory -Force -Path $RunRoot | Out-Null

    $failed = @($Results | Where-Object { $_.status -eq "failed" })
    $planned = @($Results | Where-Object { $_.status -eq "planned" })
    $passed = @($Results | Where-Object { $_.status -eq "passed" })
    $status = if ($failed.Count -gt 0) {
        "failed"
    } elseif ($planned.Count -gt 0) {
        "planned"
    } else {
        "passed"
    }

    $manifest = [ordered]@{
        mode = "local_runner"
        run_id = $RunIdValue
        created_at = (Get-Date).ToString("o")
        status = $status
        dry_run = $IsDryRun
        rerun_from_manifest = $RerunSource
        screens = @($ScreensValue)
        devices = @($DevicesValue)
        summary = [ordered]@{
            total = $Results.Count
            passed = $passed.Count
            failed = $failed.Count
            planned = $planned.Count
        }
        tasks = @($Results)
    }

    $manifestPath = Join-FullPath $RunRoot "manifest.json"
    $reportPath = Join-FullPath $RunRoot "report.md"
    $manifest | ConvertTo-Json -Depth 20 | Set-Content -Path $manifestPath -Encoding UTF8
    Build-UiAuditReport -RunRoot $RunRoot -RunIdValue $RunIdValue -Manifest $manifest | Set-Content -Path $reportPath -Encoding UTF8
}

function Format-MarkdownLink {
    param(
        [string]$Text,
        [string]$Path
    )

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return "-"
    }

    return "[$Text]($($Path -replace ' ', '%20'))"
}

function Build-UiAuditReport {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$RunIdValue,
        [Parameter(Mandatory = $true)]$Manifest
    )

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("# UI Audit Runner Report")
    $lines.Add("")
    $lines.Add("- Run ID: ``$RunIdValue``")
    $lines.Add("- Status: ``$($Manifest.status)``")
    $lines.Add("- Screens: ``$($Manifest.screens -join ', ')``")
    $lines.Add("- Devices: ``$($Manifest.devices -join ', ')``")
    $lines.Add("- Total tasks: $($Manifest.summary.total)")
    $lines.Add("- Passed: $($Manifest.summary.passed)")
    $lines.Add("- Failed: $($Manifest.summary.failed)")
    if ($Manifest.dry_run) {
        $lines.Add("- Dry run: cargo was not started")
    }
    $lines.Add("")
    $lines.Add("## Tasks")
    $lines.Add("")
    $lines.Add("| Screen | Device | States | Status | Failure | Logs | Child report |")
    $lines.Add("| --- | --- | --- | --- | --- | --- | --- |")
    foreach ($task in @($Manifest.tasks)) {
        $logs = "$(Format-MarkdownLink "stdout" $task.stdout) / $(Format-MarkdownLink "stderr" $task.stderr)"
        $childReport = Format-MarkdownLink "report" $task.child_report
        $failure = if ($task.failure_type) { "``$($task.failure_type)``" } else { "-" }
        $lines.Add("| ``$($task.screen)`` | ``$($task.device)`` | ``$($task.states)`` | ``$($task.status)`` | $failure | $logs | $childReport |")
    }

    $allCaptures = @($Manifest.tasks | ForEach-Object { $_.captures } | Where-Object { $null -ne $_ })
    if ($allCaptures.Count -gt 0) {
        $lines.Add("")
        $lines.Add("## Captures")
        foreach ($task in @($Manifest.tasks)) {
            $captures = @($task.captures)
            if ($captures.Count -eq 0) {
                continue
            }

            $lines.Add("")
            $lines.Add("### $($task.screen) / $($task.device)")
            $lines.Add("")
            $lines.Add("| State | Status | Screenshot | Metadata | Failure |")
            $lines.Add("| --- | --- | --- | --- | --- |")
            foreach ($capture in $captures) {
                $screenshotLabel = if ($capture.screenshot_exists) { "screenshot" } else { "missing screenshot" }
                $metadataLabel = if ($capture.metadata_exists) { "metadata" } else { "missing metadata" }
                $screenshot = Format-MarkdownLink $screenshotLabel $capture.screenshot
                $metadata = Format-MarkdownLink $metadataLabel $capture.metadata
                $failure = if ($capture.failure) { "``$($capture.failure)``" } else { "-" }
                $lines.Add("| ``$($capture.state)`` | ``$($capture.status)`` | $screenshot | $metadata | $failure |")
            }
        }
    }

    return ($lines.ToArray() -join [Environment]::NewLine)
}

function Assert-SelfTest {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )

    if (-not $Condition) {
        throw "Self-test failed: $Message"
    }
}

function New-FakeChildManifest {
    param(
        [Parameter(Mandatory = $true)][object]$Task,
        [Parameter(Mandatory = $true)][string]$Status,
        [string]$Failure = $null,
        [switch]$CreateArtifacts
    )

    New-Item -ItemType Directory -Force -Path $Task.output_dir | Out-Null
    $screenshot = Join-FullPath $Task.output_dir (Join-Path "screenshots" (Join-Path $Task.screen (Join-Path $Task.device "00-initial.png")))
    $metadata = Join-FullPath $Task.output_dir (Join-Path "metadata" (Join-Path $Task.screen (Join-Path $Task.device "00-initial.json")))
    if ($CreateArtifacts) {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $screenshot) | Out-Null
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $metadata) | Out-Null
        Set-Content -Path $screenshot -Value "fake-png" -Encoding ASCII
        Set-Content -Path $metadata -Value "{}" -Encoding ASCII
    }

    $entry = [ordered]@{
        screen = $Task.screen
        requested_screen = $Task.requested_screen
        device = "local-fake"
        state = "initial"
        screenshot_path = $screenshot
        metadata_path = $metadata
        scroll_target_id = $null
        scroll_position = $null
        status = $Status
        failure = $Failure
        detail = if ($Failure) { "fake failure" } else { $null }
    }
    $manifest = [ordered]@{
        mode = "local_once"
        entries = @($entry)
    }

    $manifest | ConvertTo-Json -Depth 10 | Set-Content -Path (Join-FullPath $Task.output_dir "manifest.json") -Encoding UTF8
}

function New-FakeChildManifestWithoutEntries {
    param(
        [Parameter(Mandatory = $true)][object]$Task,
        [switch]$EmptyEntries
    )

    New-Item -ItemType Directory -Force -Path $Task.output_dir | Out-Null
    $manifest = [ordered]@{
        mode = "local_once"
    }
    if ($EmptyEntries) {
        $manifest.entries = @()
    }

    $manifest | ConvertTo-Json -Depth 10 | Set-Content -Path (Join-FullPath $Task.output_dir "manifest.json") -Encoding UTF8
}

function Invoke-UiAuditSelfTest {
    $tempRoot = Join-FullPath ([System.IO.Path]::GetTempPath()) ("mybevy-ui-audit-selftest-" + [Guid]::NewGuid().ToString("N"))
    try {
        $screens = Resolve-UiAuditScreens @("ui-gallery,lobby")
        Assert-SelfTest ($screens.Count -eq 2 -and $screens[0] -eq "ui_gallery" -and $screens[1] -eq "lobby") "screen parsing and alias normalization"

        $devices = Resolve-UiAuditDevices @("phone-small", "tablet-portrait")
        Assert-SelfTest ($devices.Count -eq 2 -and $devices[0] -eq "phone-small" -and $devices[1] -eq "tablet-portrait") "device parsing"

        $extraArgs = Get-WindowArgumentOverrides -WindowProfileValue "" -WindowSizeValue "1280x2772" -DeviceScaleValue "3.25" -WindowScaleValue "50%" -RawBevyArgs @("--foo", "bar") -RawRemainingArgs @("--window-profile", "desktop")
        Assert-SelfTest (($extraArgs -join "|") -eq "--window-size|1280x2772|--device-scale|3.25|--window-scale|50%|--foo|bar|--window-profile|desktop") "window argument expansion"

        $tasks = New-UiAuditTasks -RunRoot $tempRoot -ScreensToRun $screens -DevicesToRun $devices -StateValue "auto" -ExtraBevyArgs $extraArgs
        Assert-SelfTest ($tasks.Count -eq 4) "task matrix expansion"
        Assert-SelfTest ($tasks[0].states -eq "top,middle,bottom") "ui_gallery auto states"
        Assert-SelfTest ($tasks[2].states -eq "initial") "non-recipe screen auto states"
        Assert-SelfTest (($tasks[0].bevy_args[0] -eq "--window-profile") -and ($tasks[0].bevy_args[1] -eq "phone-small")) "device window profile mapping"
        Assert-SelfTest (($tasks[0].output_dir -replace "\\", "/").Contains("/runs/ui_gallery/phone-small")) "output path layout"

        New-FakeChildManifest -Task $tasks[0] -Status "passed" -CreateArtifacts
        $passedLaunch = [pscustomobject]@{ started = $true; launch_error = $null; timed_out = $false; exit_code = 0 }
        $passed = Resolve-UiAuditTaskResult -Task $tasks[0] -LaunchResult $passedLaunch -RunRoot $tempRoot
        Assert-SelfTest ($passed.status -eq "passed") "passed classification"
        Assert-SelfTest ($passed.captures.Count -eq 1 -and $passed.captures[0].screenshot_exists) "capture artifact mapping"

        New-FakeChildManifest -Task $tasks[1] -Status "passed"
        $missing = Resolve-UiAuditTaskResult -Task $tasks[1] -LaunchResult $passedLaunch -RunRoot $tempRoot
        Assert-SelfTest ($missing.failure_type -eq "output_missing") "output missing classification"

        New-FakeChildManifestWithoutEntries -Task $tasks[1]
        $missingEntries = Resolve-UiAuditTaskResult -Task $tasks[1] -LaunchResult $passedLaunch -RunRoot $tempRoot
        Assert-SelfTest ($missingEntries.failure_type -eq "output_missing" -and $missingEntries.detail -eq "child manifest has no capture entries") "missing child manifest entries classification"

        New-FakeChildManifestWithoutEntries -Task $tasks[1] -EmptyEntries
        $emptyEntries = Resolve-UiAuditTaskResult -Task $tasks[1] -LaunchResult $passedLaunch -RunRoot $tempRoot
        Assert-SelfTest ($emptyEntries.failure_type -eq "output_missing" -and $emptyEntries.detail -eq "child manifest has no capture entries") "empty child manifest entries classification"

        New-FakeChildManifest -Task $tasks[2] -Status "failed" -Failure "screen_not_found"
        $auditFailed = Resolve-UiAuditTaskResult -Task $tasks[2] -LaunchResult $passedLaunch -RunRoot $tempRoot
        Assert-SelfTest ($auditFailed.failure_type -eq "audit_failed") "audit failure classification"

        $timeoutLaunch = [pscustomobject]@{ started = $true; launch_error = $null; timed_out = $true; exit_code = $null }
        $timeout = Resolve-UiAuditTaskResult -Task $tasks[3] -LaunchResult $timeoutLaunch -RunRoot $tempRoot
        Assert-SelfTest ($timeout.failure_type -eq "timeout") "timeout classification"

        $launchFailed = Resolve-UiAuditTaskResult -Task $tasks[3] -LaunchResult ([pscustomobject]@{ started = $false; launch_error = "fake launch failure"; timed_out = $false; exit_code = $null }) -RunRoot $tempRoot
        Assert-SelfTest ($launchFailed.failure_type -eq "launch_failed") "launch failure classification"

        $manifestMissing = Resolve-UiAuditTaskResult -Task $tasks[3] -LaunchResult $passedLaunch -RunRoot $tempRoot
        Assert-SelfTest ($manifestMissing.failure_type -eq "manifest_missing") "manifest missing classification"

        $results = @($passed, $missing, $auditFailed, $timeout)
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest" -Results $results -ScreensValue $screens -DevicesValue $devices -IsDryRun $false -RerunSource ""
        Assert-SelfTest (Test-Path (Join-FullPath $tempRoot "manifest.json")) "root manifest write"
        Assert-SelfTest (Test-Path (Join-FullPath $tempRoot "report.md")) "root report write"

        $seeds = Get-FailedTaskSeedsFromManifest -ManifestPath (Join-FullPath $tempRoot "manifest.json") -Mode "FailedOnly" -MatrixDevices $script:BasicDevices
        Assert-SelfTest ($seeds.Count -eq 3) "failed-only rerun seed expansion"
        $screenMatrix = Get-FailedTaskSeedsFromManifest -ManifestPath (Join-FullPath $tempRoot "manifest.json") -Mode "ScreenMatrix" -MatrixDevices @("desktop", "phone-small")
        Assert-SelfTest ($screenMatrix.Count -eq 4) "screen-matrix rerun seed expansion"

        Write-Host "Self-test passed."
    } finally {
        if (Test-Path $tempRoot) {
            Remove-Item -Recurse -Force -Path $tempRoot
        }
    }
}

function Invoke-UiAuditRunner {
    $scriptRoot = if (-not [string]::IsNullOrWhiteSpace($PSScriptRoot)) {
        $PSScriptRoot
    } else {
        Split-Path -Parent $PSCommandPath
    }
    $repoRoot = Get-FullPath (Join-Path $scriptRoot "..")
    $projectRoot = Join-FullPath $repoRoot "project"
    if (-not (Test-Path (Join-Path $projectRoot "Cargo.toml"))) {
        throw "Rust project root not found: $projectRoot"
    }

    $runIdValue = if ([string]::IsNullOrWhiteSpace($RunId)) { New-UiAuditRunId } else { Get-SafePathSegment $RunId }
    $outputBase = if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
        Join-FullPath $repoRoot (Join-Path "summary" "ui-audit")
    } else {
        Get-FullPath $OutputRoot
    }
    $runRoot = Join-FullPath $outputBase $runIdValue

    if (Test-Path $runRoot) {
        $existing = @(Get-ChildItem -Force -Path $runRoot)
        if ($existing.Count -gt 0) {
            throw "Run output directory already exists and is not empty: $runRoot"
        }
    }

    $extraBevyArgs = Get-WindowArgumentOverrides `
        -WindowProfileValue $WindowProfile `
        -WindowSizeValue $WindowSize `
        -DeviceScaleValue $DeviceScale `
        -WindowScaleValue $WindowScale `
        -RawBevyArgs $BevyArgs `
        -RawRemainingArgs $RemainingArgs

    $screensToRun = @()
    $devicesToRun = Resolve-UiAuditDevices $Devices
    $tasks = @()

    if (-not [string]::IsNullOrWhiteSpace($RerunFromManifest)) {
        $seeds = Get-FailedTaskSeedsFromManifest -ManifestPath (Get-FullPath $RerunFromManifest) -Mode $RerunMode -MatrixDevices $devicesToRun
        if ($seeds.Count -eq 0) {
            Write-Host "No failed screen/device tasks found in $RerunFromManifest."
            return 0
        }
        $screensToRun = @($seeds | ForEach-Object { [string]$_.screen } | Select-Object -Unique)
        $devicesToRun = @($seeds | ForEach-Object { [string]$_.device } | Select-Object -Unique)
        $tasks = New-UiAuditTasksFromSeeds -RunRoot $runRoot -Seeds $seeds -StateValue $States -ExtraBevyArgs $extraBevyArgs
    } else {
        $screensToRun = Resolve-UiAuditScreens $Screens
        $tasks = New-UiAuditTasks -RunRoot $runRoot -ScreensToRun $screensToRun -DevicesToRun $devicesToRun -StateValue $States -ExtraBevyArgs $extraBevyArgs
    }

    New-Item -ItemType Directory -Force -Path $runRoot | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $runRoot "logs") | Out-Null

    Write-Host "UI audit run: $runIdValue"
    Write-Host "Output: $runRoot"
    Write-Host "Tasks: $($tasks.Count)"

    $results = New-Object System.Collections.Generic.List[object]
    if ($DryRun) {
        foreach ($task in $tasks) {
            $results.Add((New-PlannedTaskResult -Task $task -RunRoot $runRoot))
        }
        Write-UiAuditRunnerOutputs -RunRoot $runRoot -RunIdValue $runIdValue -Results @($results.ToArray()) -ScreensValue $screensToRun -DevicesValue $devicesToRun -IsDryRun $true -RerunSource $RerunFromManifest
        Write-Host "Dry run complete. No cargo process was started."
        Write-Host "Manifest: $(Join-FullPath $runRoot "manifest.json")"
        Write-Host "Report: $(Join-FullPath $runRoot "report.md")"
        return 0
    }

    foreach ($task in $tasks) {
        Write-Host "Running $($task.screen) / $($task.device)"
        $launch = Invoke-UiAuditCargoRun -Task $task -ProjectRoot $projectRoot -TimeoutSeconds $TimeoutSeconds
        $result = Resolve-UiAuditTaskResult -Task $task -LaunchResult $launch -RunRoot $runRoot
        $results.Add($result)
        Write-UiAuditRunnerOutputs -RunRoot $runRoot -RunIdValue $runIdValue -Results @($results.ToArray()) -ScreensValue $screensToRun -DevicesValue $devicesToRun -IsDryRun $false -RerunSource $RerunFromManifest

        if ($result.status -eq "passed") {
            Write-Host "  passed"
        } else {
            Write-Host "  failed: $($result.failure_type) $($result.detail)"
        }
    }

    Write-UiAuditRunnerOutputs -RunRoot $runRoot -RunIdValue $runIdValue -Results @($results.ToArray()) -ScreensValue $screensToRun -DevicesValue $devicesToRun -IsDryRun $false -RerunSource $RerunFromManifest
    Write-Host "Manifest: $(Join-FullPath $runRoot "manifest.json")"
    Write-Host "Report: $(Join-FullPath $runRoot "report.md")"

    $failed = @($results.ToArray() | Where-Object { $_.status -eq "failed" })
    if ($failed.Count -gt 0) {
        return 1
    }
    return 0
}

if ($SelfTest) {
    Invoke-UiAuditSelfTest
    exit 0
}

$exitCode = Invoke-UiAuditRunner
exit $exitCode
