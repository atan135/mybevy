[CmdletBinding(PositionalBinding = $false)]
param(
    [ValidateSet("Local", "Remote")]
    [string]$Mode = "Local",
    [switch]$Remote,
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
    [string[]]$DeviceId = @(),
    [string[]]$ClientId = @(),
    [string[]]$SessionId = @(),
    [ValidateSet("Mock", "Http")]
    [string]$RemoteBackend = "Mock",
    [string]$AdminApiBaseUrl = "",
    [string]$AdminApiToken = "",
    [int]$RemoteCommandTimeoutMs = 5000,
    [int]$RemotePollIntervalMs = 250,
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

$script:RemoteTaskStates = @(
    "accepted",
    "queued",
    "sent",
    "running",
    "succeeded",
    "failed",
    "timeout",
    "cancelled"
)

$script:RemoteTerminalTaskStates = @(
    "succeeded",
    "failed",
    "timeout",
    "cancelled"
)

$script:RemoteUiAuditCommandTypes = @(
    "system.status",
    "ui.goto_screen",
    "ui.wait_stable",
    "ui.read_viewport",
    "ui.scroll_to",
    "ui.screenshot",
    "ui.read_tree",
    "ui.read_panels"
)

$script:RemoteKnownFailureCodes = @(
    "device_offline",
    "debug_disabled",
    "send_failed",
    "client_timeout",
    "client_rejected",
    "artifact_upload_failed"
)

function Split-UiAuditList {
    param([object[]]$Values)

    $items = New-Object System.Collections.Generic.List[string]
    foreach ($value in @($Values)) {
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

function Resolve-RemoteUiAuditTargets {
    param(
        [object[]]$InputDeviceIds,
        [object[]]$InputClientIds,
        [object[]]$InputSessionIds
    )

    $deviceIds = @(Split-UiAuditList $InputDeviceIds)
    $clientIds = @(Split-UiAuditList $InputClientIds)
    $sessionIds = @(Split-UiAuditList $InputSessionIds)

    $counts = @($deviceIds.Count, $clientIds.Count, $sessionIds.Count)
    $targetCount = ($counts | Measure-Object -Maximum).Maximum
    if ($targetCount -le 0) {
        throw "Remote mode requires at least one of -DeviceId, -ClientId, or -SessionId. -Devices remains the local window-profile matrix."
    }

    foreach ($count in $counts) {
        if ($count -gt 1 -and $count -ne $targetCount) {
            throw "Remote target lists are ambiguous. Use one value per selector or matching list lengths for -DeviceId, -ClientId, and -SessionId."
        }
    }

    $targets = New-Object System.Collections.Generic.List[object]
    for ($i = 0; $i -lt $targetCount; $i++) {
        $deviceId = if ($deviceIds.Count -eq 0) {
            $null
        } elseif ($deviceIds.Count -eq 1) {
            [string]$deviceIds[0]
        } else {
            [string]$deviceIds[$i]
        }

        $clientId = if ($clientIds.Count -eq 0) {
            $null
        } elseif ($clientIds.Count -eq 1) {
            [string]$clientIds[0]
        } else {
            [string]$clientIds[$i]
        }

        $sessionId = if ($sessionIds.Count -eq 0) {
            $null
        } elseif ($sessionIds.Count -eq 1) {
            [string]$sessionIds[0]
        } else {
            [string]$sessionIds[$i]
        }

        $parts = New-Object System.Collections.Generic.List[string]
        if (-not [string]::IsNullOrWhiteSpace($deviceId)) { $parts.Add("device_id=$deviceId") }
        if (-not [string]::IsNullOrWhiteSpace($clientId)) { $parts.Add("client_id=$clientId") }
        if (-not [string]::IsNullOrWhiteSpace($sessionId)) { $parts.Add("session_id=$sessionId") }

        $label = $parts.ToArray() -join ";"
        $targets.Add([pscustomobject]@{
            device_id = $deviceId
            client_id = $clientId
            session_id = $sessionId
            label = $label
            key = Get-SafePathSegment $label
        })
    }

    return @($targets.ToArray())
}

function Get-RemoteScrollTargetId {
    param([Parameter(Mandatory = $true)][string]$Screen)

    if ($Screen -eq "ui_gallery") {
        return "ui_gallery.main"
    }

    return "$Screen.main"
}

function Get-RemoteScrollPosition {
    param([Parameter(Mandatory = $true)][string]$State)

    if ($State -eq "middle") {
        return "middle"
    }
    if ($State -eq "bottom") {
        return "bottom"
    }

    return "top"
}

function New-RemoteUiAuditCommandSequence {
    param(
        [Parameter(Mandatory = $true)][string]$Screen,
        [Parameter(Mandatory = $true)][string]$State,
        [Parameter(Mandatory = $true)][object]$RemoteTarget,
        [Parameter(Mandatory = $true)][int]$TimeoutMs
    )

    $scrollTarget = Get-RemoteScrollTargetId -Screen $Screen
    $scrollPosition = Get-RemoteScrollPosition -State $State
    $commands = @(
        [ordered]@{
            type = "system.status"
            timeout_ms = $TimeoutMs
            payload = [ordered]@{
                audit = "ui"
                screen = $Screen
                state = $State
            }
        },
        [ordered]@{
            type = "ui.goto_screen"
            timeout_ms = $TimeoutMs
            payload = [ordered]@{
                screen = $Screen
                requested_screen = $Screen
            }
        },
        [ordered]@{
            type = "ui.wait_stable"
            timeout_ms = $TimeoutMs
            payload = [ordered]@{
                screen = $Screen
                state = $State
            }
        },
        [ordered]@{
            type = "ui.read_viewport"
            timeout_ms = $TimeoutMs
            payload = [ordered]@{
                screen = $Screen
                state = $State
            }
        },
        [ordered]@{
            type = "ui.scroll_to"
            timeout_ms = $TimeoutMs
            payload = [ordered]@{
                target = $scrollTarget
                position = $scrollPosition
                state = $State
            }
        },
        [ordered]@{
            type = "ui.screenshot"
            timeout_ms = $TimeoutMs
            payload = [ordered]@{
                label = "$Screen-$($RemoteTarget.key)-$State"
                screen = $Screen
                state = $State
            }
        },
        [ordered]@{
            type = "ui.read_tree"
            timeout_ms = $TimeoutMs
            payload = [ordered]@{
                screen = $Screen
                state = $State
            }
        },
        [ordered]@{
            type = "ui.read_panels"
            timeout_ms = $TimeoutMs
            payload = [ordered]@{
                screen = $Screen
                state = $State
            }
        }
    )

    $indexed = New-Object System.Collections.Generic.List[object]
    for ($i = 0; $i -lt $commands.Count; $i++) {
        $command = $commands[$i]
        $indexed.Add([pscustomobject]@{
            ordinal = $i + 1
            state = $State
            type = [string]$command.type
            timeout_ms = [int]$command.timeout_ms
            payload = $command.payload
        })
    }

    return @($indexed.ToArray())
}

function New-RemoteUiAuditTask {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$Screen,
        [Parameter(Mandatory = $true)][object]$RemoteTarget,
        [Parameter(Mandatory = $true)][string]$StateValue,
        [Parameter(Mandatory = $true)][int]$TimeoutMs
    )

    $screenSegment = Get-SafePathSegment $Screen
    $targetSegment = Get-SafePathSegment $RemoteTarget.key
    $outputDir = Join-FullPath $RunRoot (Join-Path "runs" (Join-Path $screenSegment $targetSegment))
    $statesForScreen = Resolve-UiAuditStates -Screen $Screen -StateValue $StateValue
    $planned = New-Object System.Collections.Generic.List[object]
    foreach ($state in (Split-UiAuditList @($statesForScreen))) {
        foreach ($command in (New-RemoteUiAuditCommandSequence -Screen $Screen -State $state -RemoteTarget $RemoteTarget -TimeoutMs $TimeoutMs)) {
            $planned.Add($command)
        }
    }

    return [pscustomobject]@{
        screen = $Screen
        requested_screen = $Screen
        device = [string]$RemoteTarget.label
        states = $statesForScreen
        output_dir = $outputDir
        remote_target = $RemoteTarget
        planned_commands = @($planned.ToArray())
    }
}

function New-RemoteUiAuditTasks {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string[]]$ScreensToRun,
        [Parameter(Mandatory = $true)][object[]]$RemoteTargets,
        [Parameter(Mandatory = $true)][string]$StateValue,
        [Parameter(Mandatory = $true)][int]$TimeoutMs
    )

    $tasks = New-Object System.Collections.Generic.List[object]
    foreach ($screen in $ScreensToRun) {
        foreach ($target in $RemoteTargets) {
            $tasks.Add((New-RemoteUiAuditTask -RunRoot $RunRoot -Screen $screen -RemoteTarget $target -StateValue $StateValue -TimeoutMs $TimeoutMs))
        }
    }
    return @($tasks.ToArray())
}

function Test-RemoteTaskStatusKnown {
    param([AllowNull()][string]$Status)

    if ([string]::IsNullOrWhiteSpace($Status)) {
        return $false
    }
    return ($script:RemoteTaskStates -contains $Status.Trim().ToLowerInvariant())
}

function Test-RemoteTaskTerminalStatus {
    param([AllowNull()][string]$Status)

    if ([string]::IsNullOrWhiteSpace($Status)) {
        return $false
    }
    return ($script:RemoteTerminalTaskStates -contains $Status.Trim().ToLowerInvariant())
}

function Convert-RemoteErrorToFailureType {
    param(
        [AllowNull()][string]$Status,
        [AllowNull()]$Error
    )

    $normalizedStatus = if ([string]::IsNullOrWhiteSpace($Status)) { "" } else { $Status.Trim().ToLowerInvariant() }
    $code = $null
    if ($null -ne $Error -and $null -ne $Error.PSObject.Properties["code"]) {
        $code = [string]$Error.code
    }
    if (-not [string]::IsNullOrWhiteSpace($code)) {
        $normalizedCode = $code.Trim().ToLowerInvariant()
        if ($script:RemoteKnownFailureCodes -contains $normalizedCode) {
            return $normalizedCode
        }
        return "remote_error"
    }

    if ($normalizedStatus -eq "timeout") {
        return "client_timeout"
    }
    if ($normalizedStatus -eq "cancelled") {
        return "cancelled"
    }
    if (-not (Test-RemoteTaskStatusKnown -Status $normalizedStatus)) {
        return "remote_status_unknown"
    }
    if ($normalizedStatus -eq "failed") {
        return "remote_failed"
    }

    return $null
}

function Convert-RemoteArtifactsToMap {
    param(
        [AllowNull()]$Artifacts,
        [Parameter(Mandatory = $true)][string]$RunRoot
    )

    $map = [ordered]@{
        screenshot = $null
        metadata = $null
        log = $null
    }

    foreach ($artifact in @($Artifacts)) {
        if ($null -eq $artifact) {
            continue
        }

        $kind = if ($null -ne $artifact.PSObject.Properties["kind"]) { [string]$artifact.kind } else { "" }
        $normalizedKind = $kind.Trim().ToLowerInvariant()
        if ($normalizedKind -eq "client_log") {
            $normalizedKind = "log"
        }
        if ($normalizedKind -notin @("screenshot", "metadata", "log")) {
            continue
        }

        $uri = if ($null -ne $artifact.PSObject.Properties["uri"]) { [string]$artifact.uri } else { $null }
        $contentType = if ($null -ne $artifact.PSObject.Properties["content_type"]) { [string]$artifact.content_type } else { $null }
        $localPath = $null
        $relativePath = $null
        $exists = $false
        if ($null -ne $artifact.PSObject.Properties["local_path"] -and -not [string]::IsNullOrWhiteSpace([string]$artifact.local_path)) {
            $candidate = Get-FullPath ([string]$artifact.local_path)
            $runRootFull = Get-FullPath $RunRoot
            if (-not $runRootFull.EndsWith([System.IO.Path]::DirectorySeparatorChar)) {
                $runRootFull = $runRootFull + [System.IO.Path]::DirectorySeparatorChar
            }
            if ($candidate.StartsWith($runRootFull, [System.StringComparison]::OrdinalIgnoreCase)) {
                $localPath = $candidate
                $relativePath = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $candidate
                $exists = Test-Path $candidate
            }
        }

        $map[$normalizedKind] = [pscustomobject]@{
            kind = $normalizedKind
            uri = $uri
            content_type = $contentType
            path = $relativePath
            exists = $exists
        }
    }

    return [pscustomobject]$map
}

function New-RemoteDebugCommandRequest {
    param(
        [Parameter(Mandatory = $true)][string]$RunIdValue,
        [Parameter(Mandatory = $true)][object]$Task,
        [Parameter(Mandatory = $true)][object]$Command,
        [Parameter(Mandatory = $true)][int]$GlobalOrdinal
    )

    $requestIdSeed = "uiaudit-$RunIdValue-$($Task.screen)-$($Task.remote_target.key)-$($Command.state)-$GlobalOrdinal-$($Command.type)"
    $request = [ordered]@{
        request_id = Get-SafePathSegment $requestIdSeed
        device_id = $Task.remote_target.device_id
        session_id = $Task.remote_target.session_id
        client_id = $Task.remote_target.client_id
        command = [ordered]@{
            type = [string]$Command.type
            timeout_ms = [int]$Command.timeout_ms
            payload = $Command.payload
        }
        wait = [ordered]@{
            enabled = $false
            timeout_ms = 0
        }
    }

    return [pscustomobject]$request
}

$script:MockRemoteDebugTaskStore = @{}
$script:MockRemoteDebugTaskCounter = 0

function Initialize-MockRemoteAdminApi {
    $script:MockRemoteDebugTaskStore = @{}
    $script:MockRemoteDebugTaskCounter = 0
}

function Get-MockRemoteFailureCode {
    param(
        [Parameter(Mandatory = $true)][object]$Request,
        [Parameter(Mandatory = $true)][object]$Command
    )

    if ($null -ne $Command.payload -and $null -ne $Command.payload.PSObject.Properties["mock_error_code"]) {
        return [string]$Command.payload.mock_error_code
    }

    $targetValues = @([string]$Request.device_id, [string]$Request.client_id, [string]$Request.session_id)
    foreach ($value in $targetValues) {
        if ([string]::IsNullOrWhiteSpace($value)) {
            continue
        }
        if ($value.StartsWith("mock-fail-", [System.StringComparison]::OrdinalIgnoreCase)) {
            return $value.Substring("mock-fail-".Length)
        }
    }

    return $null
}

function Get-MockRemoteArtifactMode {
    param(
        [Parameter(Mandatory = $true)][object]$Request,
        [Parameter(Mandatory = $true)][object]$Command
    )

    if ($null -ne $Command.payload -and $null -ne $Command.payload.PSObject.Properties["mock_artifact_mode"]) {
        return ([string]$Command.payload.mock_artifact_mode).Trim().ToLowerInvariant()
    }

    $targetValues = @([string]$Request.device_id, [string]$Request.client_id, [string]$Request.session_id)
    foreach ($value in $targetValues) {
        if ([string]::IsNullOrWhiteSpace($value)) {
            continue
        }
        if ($value.StartsWith("mock-artifacts-", [System.StringComparison]::OrdinalIgnoreCase)) {
            return $value.Substring("mock-artifacts-".Length).Trim().ToLowerInvariant()
        }
    }

    return "complete"
}

function New-MockRemoteArtifactFiles {
    param(
        [Parameter(Mandatory = $true)][string]$TaskId,
        [Parameter(Mandatory = $true)][string]$TaskOutputDir,
        [Parameter(Mandatory = $true)][object]$Command,
        [string]$ArtifactMode = "complete"
    )

    $artifactDir = Join-FullPath $TaskOutputDir (Join-Path "artifacts" $TaskId)
    New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null

    $screenshotPath = Join-FullPath $artifactDir "screenshot.png"
    $metadataPath = Join-FullPath $artifactDir "metadata.json"
    $logPath = Join-FullPath $artifactDir "client.log"

    $normalizedMode = if ([string]::IsNullOrWhiteSpace($ArtifactMode)) { "complete" } else { $ArtifactMode.Trim().ToLowerInvariant() }

    $metadata = [ordered]@{
        mock = $true
        task_id = $TaskId
        command_type = [string]$Command.type
        screen = [string]$Command.payload.screen
        state = [string]$Command.payload.state
        viewport = [ordered]@{
            logical_width = 360
            logical_height = 800
            safe_area = [ordered]@{
                left = 0
                right = 0
                top = 32
                bottom = 24
            }
        }
    }

    $artifacts = New-Object System.Collections.Generic.List[object]
    if ($normalizedMode -notin @("empty", "missing_screenshot")) {
        Set-Content -Path $screenshotPath -Value "mock screenshot for $TaskId" -Encoding ASCII
        $artifacts.Add([pscustomobject]@{
            kind = "screenshot"
            uri = "artifact://debug/$TaskId/screenshot.png"
            content_type = "image/png"
            local_path = $screenshotPath
        })
    }
    if ($normalizedMode -notin @("empty", "missing_metadata")) {
        $metadata | ConvertTo-Json -Depth 10 | Set-Content -Path $metadataPath -Encoding UTF8
        $artifacts.Add([pscustomobject]@{
            kind = "metadata"
            uri = "artifact://debug/$TaskId/metadata.json"
            content_type = "application/json"
            local_path = $metadataPath
        })
    }
    if ($normalizedMode -ne "empty") {
        Set-Content -Path $logPath -Value "mock client log for $TaskId" -Encoding UTF8
        $artifacts.Add([pscustomobject]@{
            kind = "client_log"
            uri = "artifact://debug/$TaskId/client.log"
            content_type = "text/plain"
            local_path = $logPath
        })
    }

    return @($artifacts.ToArray())
}

function New-MockRemoteDebugTask {
    param(
        [Parameter(Mandatory = $true)][object]$Request,
        [Parameter(Mandatory = $true)][string]$TaskOutputDir
    )

    $script:MockRemoteDebugTaskCounter += 1
    $taskId = "dbg_task_mock_{0:D4}" -f $script:MockRemoteDebugTaskCounter
    $command = $Request.command
    $commandType = [string]$command.type
    $failureCode = Get-MockRemoteFailureCode -Request $Request -Command $command
    $artifactMode = Get-MockRemoteArtifactMode -Request $Request -Command $command

    if ($script:RemoteUiAuditCommandTypes -notcontains $commandType) {
        $failureCode = "unknown_command"
    }

    $finalStatus = if ([string]::IsNullOrWhiteSpace($failureCode)) { "succeeded" } elseif ($failureCode -eq "client_timeout") { "timeout" } else { "failed" }
    $artifacts = @()
    if ($finalStatus -eq "succeeded" -and $commandType -eq "ui.screenshot") {
        $artifacts = @(New-MockRemoteArtifactFiles -TaskId $taskId -TaskOutputDir $TaskOutputDir -Command $command -ArtifactMode $artifactMode)
    }

    $errorObject = $null
    if ($finalStatus -ne "succeeded") {
        $errorObject = [pscustomobject]@{
            code = $failureCode
            message = "mock remote failure: $failureCode"
            retryable = ($failureCode -in @("device_offline", "send_failed", "client_timeout", "artifact_upload_failed"))
        }
    }

    $resultObject = if ($finalStatus -eq "succeeded") {
        [pscustomobject]@{
            command_type = $commandType
            width = 1080
            height = 2400
            viewport = [pscustomobject]@{
                logical_width = 360
                logical_height = 800
            }
        }
    } else {
        $null
    }

    $script:MockRemoteDebugTaskStore[$taskId] = [pscustomobject]@{
        task_id = $taskId
        request_id = [string]$Request.request_id
        device_id = [string]$Request.device_id
        client_id = [string]$Request.client_id
        session_id = [string]$Request.session_id
        command_type = $commandType
        flow = @("accepted", "queued", "sent", "running", $finalStatus)
        poll_index = 0
        result = $resultObject
        artifacts = @($artifacts)
        error = $errorObject
    }

    return [pscustomobject]@{
        ok = $true
        task_id = $taskId
        status = "accepted"
    }
}

function Get-MockRemoteDebugTask {
    param([Parameter(Mandatory = $true)][string]$TaskId)

    if (-not $script:MockRemoteDebugTaskStore.ContainsKey($TaskId)) {
        return [pscustomobject]@{
            ok = $false
            task_id = $TaskId
            status = "failed"
            command_type = $null
            result = $null
            artifacts = @()
            error = [pscustomobject]@{
                code = "unknown_task"
                message = "mock task was not found"
                retryable = $false
            }
        }
    }

    $entry = $script:MockRemoteDebugTaskStore[$TaskId]
    $flow = @($entry.flow)
    $index = [Math]::Min([int]$entry.poll_index, $flow.Count - 1)
    $status = [string]$flow[$index]
    if ([int]$entry.poll_index -lt ($flow.Count - 1)) {
        $entry.poll_index = [int]$entry.poll_index + 1
    }
    $terminal = Test-RemoteTaskTerminalStatus -Status $status

    return [pscustomobject]@{
        ok = $true
        task_id = [string]$entry.task_id
        request_id = [string]$entry.request_id
        device_id = [string]$entry.device_id
        client_id = [string]$entry.client_id
        session_id = [string]$entry.session_id
        status = $status
        command_type = [string]$entry.command_type
        result = if ($terminal) { $entry.result } else { $null }
        artifacts = if ($terminal) { @($entry.artifacts) } else { @() }
        error = if ($terminal) { $entry.error } else { $null }
    }
}

function Invoke-RemoteDebugCreateTask {
    param(
        [Parameter(Mandatory = $true)][string]$Backend,
        [Parameter(Mandatory = $true)][object]$Request,
        [Parameter(Mandatory = $true)][string]$TaskOutputDir,
        [AllowNull()][string]$BaseUrl,
        [AllowNull()][string]$Token
    )

    if ($Backend -eq "Mock") {
        return New-MockRemoteDebugTask -Request $Request -TaskOutputDir $TaskOutputDir
    }

    if ([string]::IsNullOrWhiteSpace($BaseUrl)) {
        throw "-AdminApiBaseUrl is required when -RemoteBackend Http is used."
    }

    $headers = @{}
    if (-not [string]::IsNullOrWhiteSpace($Token)) {
        $headers["Authorization"] = "Bearer $Token"
    }
    $uri = "$($BaseUrl.TrimEnd('/'))/admin/debug/commands"
    return Invoke-RestMethod -Method Post -Uri $uri -Headers $headers -ContentType "application/json" -Body ($Request | ConvertTo-Json -Depth 20)
}

function Get-RemoteDebugTask {
    param(
        [Parameter(Mandatory = $true)][string]$Backend,
        [Parameter(Mandatory = $true)][string]$TaskId,
        [AllowNull()][string]$BaseUrl,
        [AllowNull()][string]$Token
    )

    if ($Backend -eq "Mock") {
        return Get-MockRemoteDebugTask -TaskId $TaskId
    }

    if ([string]::IsNullOrWhiteSpace($BaseUrl)) {
        throw "-AdminApiBaseUrl is required when -RemoteBackend Http is used."
    }

    $headers = @{}
    if (-not [string]::IsNullOrWhiteSpace($Token)) {
        $headers["Authorization"] = "Bearer $Token"
    }
    $uri = "$($BaseUrl.TrimEnd('/'))/admin/debug/tasks/$TaskId"
    return Invoke-RestMethod -Method Get -Uri $uri -Headers $headers
}

function Wait-RemoteDebugTask {
    param(
        [Parameter(Mandatory = $true)][string]$Backend,
        [Parameter(Mandatory = $true)][string]$TaskId,
        [Parameter(Mandatory = $true)][int]$TimeoutMs,
        [Parameter(Mandatory = $true)][int]$PollIntervalMs,
        [AllowNull()][string]$BaseUrl,
        [AllowNull()][string]$Token
    )

    $started = Get-Date
    $lastTask = $null
    while ($true) {
        $lastTask = Get-RemoteDebugTask -Backend $Backend -TaskId $TaskId -BaseUrl $BaseUrl -Token $Token
        $status = if ($null -ne $lastTask -and $null -ne $lastTask.PSObject.Properties["status"]) { [string]$lastTask.status } else { "" }
        if (Test-RemoteTaskTerminalStatus -Status $status) {
            return $lastTask
        }

        $elapsedMs = ((Get-Date) - $started).TotalMilliseconds
        if ($elapsedMs -ge $TimeoutMs) {
            return [pscustomobject]@{
                ok = $false
                task_id = $TaskId
                request_id = if ($lastTask -and $lastTask.PSObject.Properties["request_id"]) { [string]$lastTask.request_id } else { $null }
                device_id = if ($lastTask -and $lastTask.PSObject.Properties["device_id"]) { [string]$lastTask.device_id } else { $null }
                status = "timeout"
                command_type = if ($lastTask -and $lastTask.PSObject.Properties["command_type"]) { [string]$lastTask.command_type } else { $null }
                result = $null
                artifacts = @()
                error = [pscustomobject]@{
                    code = "client_timeout"
                    message = "remote debug task did not reach a terminal state before runner timeout"
                    retryable = $true
                }
            }
        }

        if ($Backend -ne "Mock" -and $PollIntervalMs -gt 0) {
            Start-Sleep -Milliseconds $PollIntervalMs
        }
    }
}

function Invoke-RemoteDebugCommand {
    param(
        [Parameter(Mandatory = $true)][string]$Backend,
        [Parameter(Mandatory = $true)][object]$Request,
        [Parameter(Mandatory = $true)][string]$TaskOutputDir,
        [Parameter(Mandatory = $true)][int]$PollIntervalMs,
        [AllowNull()][string]$BaseUrl,
        [AllowNull()][string]$Token
    )

    try {
        $created = Invoke-RemoteDebugCreateTask -Backend $Backend -Request $Request -TaskOutputDir $TaskOutputDir -BaseUrl $BaseUrl -Token $Token
    } catch {
        return [pscustomobject]@{
            ok = $false
            task_id = $null
            request_id = [string]$Request.request_id
            device_id = [string]$Request.device_id
            client_id = [string]$Request.client_id
            session_id = [string]$Request.session_id
            status = "failed"
            command_type = [string]$Request.command.type
            result = $null
            artifacts = @()
            error = [pscustomobject]@{
                code = "adminapi_request_failed"
                message = $_.Exception.Message
                retryable = $true
            }
        }
    }

    $taskId = if ($null -ne $created.PSObject.Properties["task_id"]) { [string]$created.task_id } else { $null }
    if ([string]::IsNullOrWhiteSpace($taskId)) {
        return [pscustomobject]@{
            ok = $false
            task_id = $null
            request_id = [string]$Request.request_id
            device_id = [string]$Request.device_id
            client_id = [string]$Request.client_id
            session_id = [string]$Request.session_id
            status = "failed"
            command_type = [string]$Request.command.type
            result = $null
            artifacts = @()
            error = [pscustomobject]@{
                code = "invalid_response"
                message = "adminapi create response did not include task_id"
                retryable = $true
            }
        }
    }

    return Wait-RemoteDebugTask -Backend $Backend -TaskId $taskId -TimeoutMs ([int]$Request.command.timeout_ms) -PollIntervalMs $PollIntervalMs -BaseUrl $BaseUrl -Token $Token
}

function Convert-RemoteDebugTaskToRecord {
    param(
        [Parameter(Mandatory = $true)][object]$Request,
        [Parameter(Mandatory = $true)][object]$TaskResult,
        [Parameter(Mandatory = $true)][string]$RunRoot
    )

    $artifacts = Convert-RemoteArtifactsToMap -Artifacts $TaskResult.artifacts -RunRoot $RunRoot
    return [pscustomobject]@{
        request_id = [string]$Request.request_id
        task_id = if ($null -ne $TaskResult.PSObject.Properties["task_id"]) { [string]$TaskResult.task_id } else { $null }
        command_type = [string]$Request.command.type
        status = if ($null -ne $TaskResult.PSObject.Properties["status"]) { [string]$TaskResult.status } else { "failed" }
        failure_type = Convert-RemoteErrorToFailureType -Status ([string]$TaskResult.status) -Error $TaskResult.error
        error = $TaskResult.error
        artifacts = $artifacts
        artifact_uris = @($TaskResult.artifacts | ForEach-Object { if ($null -ne $_.PSObject.Properties["uri"]) { [string]$_.uri } })
        result = $TaskResult.result
    }
}

function Get-MissingRequiredRemoteScreenshotArtifacts {
    param([AllowNull()]$Artifacts)

    $missing = New-Object System.Collections.Generic.List[string]
    if ($null -eq $Artifacts -or $null -eq $Artifacts.screenshot -or [string]::IsNullOrWhiteSpace([string]$Artifacts.screenshot.uri)) {
        $missing.Add("screenshot")
    }
    if ($null -eq $Artifacts -or $null -eq $Artifacts.metadata -or [string]::IsNullOrWhiteSpace([string]$Artifacts.metadata.uri)) {
        $missing.Add("metadata")
    }

    return @($missing.ToArray())
}

function New-RemoteCapture {
    param(
        [Parameter(Mandatory = $true)][object]$Task,
        [Parameter(Mandatory = $true)][string]$State,
        [Parameter(Mandatory = $true)][string]$Status,
        [AllowNull()][string]$Failure,
        [AllowNull()][string]$Detail,
        [AllowNull()]$ScreenshotArtifact,
        [AllowNull()]$MetadataArtifact,
        [AllowNull()]$LogArtifact,
        [AllowNull()][string]$ScreenshotTaskId,
        [AllowNull()][string]$MetadataTaskId,
        [AllowNull()][string]$LogTaskId,
        [AllowNull()][object[]]$StateTaskIds
    )

    return [pscustomobject]@{
        screen = [string]$Task.screen
        requested_screen = [string]$Task.requested_screen
        device = [string]$Task.device
        remote_target = $Task.remote_target
        state = $State
        status = $Status
        failure = $Failure
        detail = $Detail
        screenshot = if ($ScreenshotArtifact) { $ScreenshotArtifact.path } else { $null }
        metadata = if ($MetadataArtifact) { $MetadataArtifact.path } else { $null }
        log = if ($LogArtifact) { $LogArtifact.path } else { $null }
        screenshot_exists = if ($ScreenshotArtifact) { [bool]$ScreenshotArtifact.exists } else { $false }
        metadata_exists = if ($MetadataArtifact) { [bool]$MetadataArtifact.exists } else { $false }
        log_exists = if ($LogArtifact) { [bool]$LogArtifact.exists } else { $false }
        screenshot_artifact_uri = if ($ScreenshotArtifact) { [string]$ScreenshotArtifact.uri } else { $null }
        metadata_artifact_uri = if ($MetadataArtifact) { [string]$MetadataArtifact.uri } else { $null }
        log_artifact_uri = if ($LogArtifact) { [string]$LogArtifact.uri } else { $null }
        screenshot_task_id = $ScreenshotTaskId
        metadata_task_id = $MetadataTaskId
        log_task_id = $LogTaskId
        remote_task_ids = @($StateTaskIds)
        scroll_target_id = Get-RemoteScrollTargetId -Screen ([string]$Task.screen)
        scroll_position = Get-RemoteScrollPosition -State $State
    }
}

function New-PlannedRemoteTaskResult {
    param(
        [Parameter(Mandatory = $true)][object]$Task,
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$RunIdValue
    )

    $plannedRequests = New-Object System.Collections.Generic.List[object]
    $ordinal = 0
    foreach ($command in @($Task.planned_commands)) {
        $ordinal += 1
        $plannedRequests.Add((New-RemoteDebugCommandRequest -RunIdValue $RunIdValue -Task $Task -Command $command -GlobalOrdinal $ordinal))
    }

    return [pscustomobject]@{
        screen = [string]$Task.screen
        requested_screen = [string]$Task.requested_screen
        device = [string]$Task.device
        states = [string]$Task.states
        status = "planned"
        failure_type = $null
        detail = "dry run; remote adminapi tasks were not created"
        output_dir = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path ([string]$Task.output_dir)
        remote_target = $Task.remote_target
        planned_commands = @($Task.planned_commands)
        planned_requests = @($plannedRequests.ToArray())
        remote_tasks = @()
        task_ids = @()
        request_ids = @($plannedRequests.ToArray() | ForEach-Object { [string]$_.request_id })
        captures = @()
    }
}

function Invoke-RemoteUiAuditTask {
    param(
        [Parameter(Mandatory = $true)][object]$Task,
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$RunIdValue,
        [Parameter(Mandatory = $true)][string]$Backend,
        [AllowNull()][string]$BaseUrl,
        [AllowNull()][string]$Token,
        [Parameter(Mandatory = $true)][int]$PollIntervalMs
    )

    New-Item -ItemType Directory -Force -Path $Task.output_dir | Out-Null

    $remoteTaskRecords = New-Object System.Collections.Generic.List[object]
    $captures = New-Object System.Collections.Generic.List[object]
    $allTaskIds = New-Object System.Collections.Generic.List[string]
    $allRequestIds = New-Object System.Collections.Generic.List[string]
    $globalOrdinal = 0

    $base = [ordered]@{
        screen = [string]$Task.screen
        requested_screen = [string]$Task.requested_screen
        device = [string]$Task.device
        states = [string]$Task.states
        status = "failed"
        failure_type = $null
        detail = $null
        output_dir = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path ([string]$Task.output_dir)
        remote_target = $Task.remote_target
        planned_commands = @($Task.planned_commands)
        remote_tasks = @()
        task_ids = @()
        request_ids = @()
        captures = @()
    }

    foreach ($state in (Split-UiAuditList @($Task.states))) {
        $stateTaskIds = New-Object System.Collections.Generic.List[string]
        $screenshotArtifact = $null
        $metadataArtifact = $null
        $logArtifact = $null
        $screenshotTaskId = $null
        $metadataTaskId = $null
        $logTaskId = $null
        $stateFailed = $false

        $commands = @(New-RemoteUiAuditCommandSequence -Screen ([string]$Task.screen) -State $state -RemoteTarget $Task.remote_target -TimeoutMs $RemoteCommandTimeoutMs)
        foreach ($command in $commands) {
            $globalOrdinal += 1
            $request = New-RemoteDebugCommandRequest -RunIdValue $RunIdValue -Task $Task -Command $command -GlobalOrdinal $globalOrdinal
            $allRequestIds.Add([string]$request.request_id)
            $taskResult = Invoke-RemoteDebugCommand -Backend $Backend -Request $request -TaskOutputDir ([string]$Task.output_dir) -PollIntervalMs $PollIntervalMs -BaseUrl $BaseUrl -Token $Token
            $record = Convert-RemoteDebugTaskToRecord -Request $request -TaskResult $taskResult -RunRoot $RunRoot
            $remoteTaskRecords.Add($record)
            if (-not [string]::IsNullOrWhiteSpace([string]$record.task_id)) {
                $allTaskIds.Add([string]$record.task_id)
                $stateTaskIds.Add([string]$record.task_id)
            }

            if ($record.command_type -eq "ui.screenshot" -and $record.status -eq "succeeded") {
                $screenshotArtifact = $record.artifacts.screenshot
                $metadataArtifact = $record.artifacts.metadata
                $logArtifact = $record.artifacts.log
                $screenshotTaskId = [string]$record.task_id
                $metadataTaskId = [string]$record.task_id
                $logTaskId = [string]$record.task_id

                $missingArtifacts = @(Get-MissingRequiredRemoteScreenshotArtifacts -Artifacts $record.artifacts)
                if ($missingArtifacts.Count -gt 0) {
                    $failureType = "artifact_upload_failed"
                    $detail = "ui.screenshot succeeded but missing required artifact URI(s): $($missingArtifacts -join ', ')"
                    $captures.Add((New-RemoteCapture -Task $Task -State $state -Status "failed" -Failure $failureType -Detail $detail -ScreenshotArtifact $screenshotArtifact -MetadataArtifact $metadataArtifact -LogArtifact $logArtifact -ScreenshotTaskId $screenshotTaskId -MetadataTaskId $metadataTaskId -LogTaskId $logTaskId -StateTaskIds @($stateTaskIds.ToArray())))
                    $base.failure_type = $failureType
                    $base.detail = $detail
                    $stateFailed = $true
                    break
                }
            }

            if ($record.status -ne "succeeded") {
                $failureType = if ($record.failure_type) { [string]$record.failure_type } else { "remote_failed" }
                $detail = if ($null -ne $record.error -and $null -ne $record.error.PSObject.Properties["message"]) {
                    [string]$record.error.message
                } else {
                    "remote task $($record.task_id) ended with status $($record.status)"
                }

                $captures.Add((New-RemoteCapture -Task $Task -State $state -Status "failed" -Failure $failureType -Detail $detail -ScreenshotArtifact $screenshotArtifact -MetadataArtifact $metadataArtifact -LogArtifact $logArtifact -ScreenshotTaskId $screenshotTaskId -MetadataTaskId $metadataTaskId -LogTaskId $logTaskId -StateTaskIds @($stateTaskIds.ToArray())))
                $base.failure_type = $failureType
                $base.detail = "$($record.command_type): $detail"
                $stateFailed = $true
                break
            }
        }

        if ($stateFailed) {
            break
        }

        $captures.Add((New-RemoteCapture -Task $Task -State $state -Status "passed" -Failure $null -Detail $null -ScreenshotArtifact $screenshotArtifact -MetadataArtifact $metadataArtifact -LogArtifact $logArtifact -ScreenshotTaskId $screenshotTaskId -MetadataTaskId $metadataTaskId -LogTaskId $logTaskId -StateTaskIds @($stateTaskIds.ToArray())))
    }

    $base.remote_tasks = @($remoteTaskRecords.ToArray())
    $base.task_ids = @($allTaskIds.ToArray())
    $base.request_ids = @($allRequestIds.ToArray())
    $base.captures = @($captures.ToArray())

    if ($base.failure_type) {
        return [pscustomobject]$base
    }

    $failedCaptures = @($captures.ToArray() | Where-Object { $_.status -ne "passed" })
    if ($failedCaptures.Count -gt 0) {
        $base.failure_type = "remote_failed"
        $base.detail = "one or more remote captures failed"
        return [pscustomobject]$base
    }

    $base.status = "passed"
    return [pscustomobject]$base
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
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$RerunSource,
        [ValidateSet("Local", "Remote")]
        [string]$RunnerMode = "Local",
        [object[]]$RemoteTargetsValue = @(),
        [AllowEmptyString()][string]$RemoteBackendName = "",
        [string[]]$LocalDevicesValue = @()
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

    $isRemote = $RunnerMode -eq "Remote"
    $remoteTargetsForManifest = @()
    if ($isRemote) {
        $remoteTargetsForManifest = @($RemoteTargetsValue)
    }
    $localDevicesForManifest = @($LocalDevicesValue)
    $manifest = [ordered]@{
        mode = if ($isRemote) { "remote_runner" } else { "local_runner" }
        runner_mode = if ($isRemote) { "remote" } else { "local" }
        run_id = $RunIdValue
        created_at = (Get-Date).ToString("o")
        status = $status
        dry_run = $IsDryRun
        rerun_from_manifest = $RerunSource
        screens = @($ScreensValue)
        devices = @($DevicesValue)
        local_devices = $localDevicesForManifest
        remote_backend = if ($isRemote) { $RemoteBackendName } else { $null }
        remote_targets = $remoteTargetsForManifest
        execution_priority = [ordered]@{
            local = "desktop development and CI fallback"
            remote = "multi-device, mobile, and AI interactive audit primary channel when explicitly selected"
            selected = if ($isRemote) { "remote" } else { "local" }
        }
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

function Format-MarkdownCodeOrDash {
    param([AllowNull()][string]$Value)

    if ([string]::IsNullOrWhiteSpace($Value)) {
        return "-"
    }

    return "``$Value``"
}

function Format-ArtifactReference {
    param(
        [AllowNull()][string]$Path,
        [AllowNull()][string]$Uri,
        [Parameter(Mandatory = $true)][string]$Label
    )

    if (-not [string]::IsNullOrWhiteSpace($Path)) {
        return Format-MarkdownLink $Label $Path
    }

    if (-not [string]::IsNullOrWhiteSpace($Uri)) {
        return "``$Uri``"
    }

    return "-"
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
    $lines.Add("- Mode: ``$($Manifest.runner_mode)``")
    $lines.Add("- Screens: ``$($Manifest.screens -join ', ')``")
    $lines.Add("- Devices: ``$($Manifest.devices -join ', ')``")
    $lines.Add("- Total tasks: $($Manifest.summary.total)")
    $lines.Add("- Passed: $($Manifest.summary.passed)")
    $lines.Add("- Failed: $($Manifest.summary.failed)")
    if ($Manifest.dry_run) {
        $dryRunDetail = if ($Manifest.runner_mode -eq "remote") {
            "remote adminapi tasks were not created"
        } else {
            "cargo was not started"
        }
        $lines.Add("- Dry run: $dryRunDetail")
    }
    if ($Manifest.runner_mode -eq "remote") {
        $lines.Add("- Remote backend: ``$($Manifest.remote_backend)``")
        $lines.Add("- Local fallback devices: ``$($Manifest.local_devices -join ', ')``")
        $lines.Add("- Channel priority: remote primary when explicitly selected; local remains desktop/CI fallback")
    }
    $lines.Add("")
    $lines.Add("## Tasks")
    $lines.Add("")

    if ($Manifest.runner_mode -eq "remote") {
        $lines.Add("| Screen | Remote target | States | Status | Failure | Task IDs | Screenshot artifacts |")
        $lines.Add("| --- | --- | --- | --- | --- | --- | --- |")
        foreach ($task in @($Manifest.tasks)) {
            $failure = if ($task.failure_type) { "``$($task.failure_type)``" } else { "-" }
            $taskIds = if ($task.task_ids -and @($task.task_ids).Count -gt 0) {
                "``$((@($task.task_ids) | Select-Object -First 6) -join ', ')``"
            } else {
                "-"
            }
            $screenshotUris = @($task.captures | ForEach-Object { [string]$_.screenshot_artifact_uri } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Unique)
            $artifactText = if ($screenshotUris.Count -gt 0) {
                "``$($screenshotUris -join ', ')``"
            } else {
                "-"
            }
            $lines.Add("| ``$($task.screen)`` | ``$($task.device)`` | ``$($task.states)`` | ``$($task.status)`` | $failure | $taskIds | $artifactText |")
        }
    } else {
        $lines.Add("| Screen | Device | States | Status | Failure | Logs | Child report |")
        $lines.Add("| --- | --- | --- | --- | --- | --- | --- |")
        foreach ($task in @($Manifest.tasks)) {
            $logs = "$(Format-MarkdownLink "stdout" $task.stdout) / $(Format-MarkdownLink "stderr" $task.stderr)"
            $childReport = Format-MarkdownLink "report" $task.child_report
            $failure = if ($task.failure_type) { "``$($task.failure_type)``" } else { "-" }
            $lines.Add("| ``$($task.screen)`` | ``$($task.device)`` | ``$($task.states)`` | ``$($task.status)`` | $failure | $logs | $childReport |")
        }
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
            if ($Manifest.runner_mode -eq "remote") {
                $lines.Add("| State | Status | Screenshot | Metadata | Log | Screenshot artifact | Task IDs | Failure |")
                $lines.Add("| --- | --- | --- | --- | --- | --- | --- | --- |")
                foreach ($capture in $captures) {
                    $screenshot = Format-ArtifactReference -Path $capture.screenshot -Uri $capture.screenshot_artifact_uri -Label "screenshot"
                    $metadata = Format-ArtifactReference -Path $capture.metadata -Uri $capture.metadata_artifact_uri -Label "metadata"
                    $log = Format-ArtifactReference -Path $capture.log -Uri $capture.log_artifact_uri -Label "log"
                    $artifact = Format-MarkdownCodeOrDash $capture.screenshot_artifact_uri
                    $taskIds = if ($capture.remote_task_ids -and @($capture.remote_task_ids).Count -gt 0) {
                        "``$(@($capture.remote_task_ids) -join ', ')``"
                    } else {
                        "-"
                    }
                    $failure = if ($capture.failure) { "``$($capture.failure)``" } else { "-" }
                    $lines.Add("| ``$($capture.state)`` | ``$($capture.status)`` | $screenshot | $metadata | $log | $artifact | $taskIds | $failure |")
                }
            } else {
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
        $screens = @(Resolve-UiAuditScreens @("ui-gallery,lobby"))
        Assert-SelfTest ($screens.Count -eq 2 -and $screens[0] -eq "ui_gallery" -and $screens[1] -eq "lobby") "screen parsing and alias normalization"

        $devices = @(Resolve-UiAuditDevices @("phone-small", "tablet-portrait"))
        Assert-SelfTest ($devices.Count -eq 2 -and $devices[0] -eq "phone-small" -and $devices[1] -eq "tablet-portrait") "device parsing"

        $extraArgs = Get-WindowArgumentOverrides -WindowProfileValue "" -WindowSizeValue "1280x2772" -DeviceScaleValue "3.25" -WindowScaleValue "50%" -RawBevyArgs @("--foo", "bar") -RawRemainingArgs @("--window-profile", "desktop")
        Assert-SelfTest (($extraArgs -join "|") -eq "--window-size|1280x2772|--device-scale|3.25|--window-scale|50%|--foo|bar|--window-profile|desktop") "window argument expansion"

        $tasks = @(New-UiAuditTasks -RunRoot $tempRoot -ScreensToRun $screens -DevicesToRun $devices -StateValue "auto" -ExtraBevyArgs $extraArgs)
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
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest" -Results $results -ScreensValue $screens -DevicesValue $devices -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue $devices
        Assert-SelfTest (Test-Path (Join-FullPath $tempRoot "manifest.json")) "root manifest write"
        Assert-SelfTest (Test-Path (Join-FullPath $tempRoot "report.md")) "root report write"

        $seeds = Get-FailedTaskSeedsFromManifest -ManifestPath (Join-FullPath $tempRoot "manifest.json") -Mode "FailedOnly" -MatrixDevices $script:BasicDevices
        Assert-SelfTest ($seeds.Count -eq 3) "failed-only rerun seed expansion"
        $screenMatrix = Get-FailedTaskSeedsFromManifest -ManifestPath (Join-FullPath $tempRoot "manifest.json") -Mode "ScreenMatrix" -MatrixDevices @("desktop", "phone-small")
        Assert-SelfTest ($screenMatrix.Count -eq 4) "screen-matrix rerun seed expansion"

        $remoteTargets = @(Resolve-RemoteUiAuditTargets -InputDeviceIds @("android-test-01") -InputClientIds @("client-123") -InputSessionIds @("session-abc"))
        Assert-SelfTest ($remoteTargets.Count -eq 1 -and $remoteTargets[0].device_id -eq "android-test-01" -and $remoteTargets[0].client_id -eq "client-123") "remote target parsing"

        $remoteCommands = @(New-RemoteUiAuditCommandSequence -Screen "ui_gallery" -State "middle" -RemoteTarget $remoteTargets[0] -TimeoutMs 5000)
        Assert-SelfTest (($remoteCommands | ForEach-Object { $_.type }) -join "," -eq "system.status,ui.goto_screen,ui.wait_stable,ui.read_viewport,ui.scroll_to,ui.screenshot,ui.read_tree,ui.read_panels") "remote command sequence"
        Assert-SelfTest ($remoteCommands[4].payload.position -eq "middle" -and $remoteCommands[4].payload.target -eq "ui_gallery.main") "remote scroll command payload"

        Assert-SelfTest (Test-RemoteTaskStatusKnown -Status "accepted") "remote accepted state known"
        Assert-SelfTest (Test-RemoteTaskTerminalStatus -Status "succeeded") "remote succeeded terminal"
        Assert-SelfTest (-not (Test-RemoteTaskTerminalStatus -Status "running")) "remote running non-terminal"
        foreach ($code in $script:RemoteKnownFailureCodes) {
            $failureType = Convert-RemoteErrorToFailureType -Status "failed" -Error ([pscustomobject]@{ code = $code; message = "x"; retryable = $false })
            Assert-SelfTest ($failureType -eq $code) "remote failure classification for $code"
        }
        Assert-SelfTest ((Convert-RemoteErrorToFailureType -Status "timeout" -Error $null) -eq "client_timeout") "remote timeout classification"
        Assert-SelfTest ((Convert-RemoteErrorToFailureType -Status "mystery" -Error $null) -eq "remote_status_unknown") "remote unknown status classification"

        $remoteTask = New-RemoteUiAuditTask -RunRoot $tempRoot -Screen "ui_gallery" -RemoteTarget $remoteTargets[0] -StateValue "top,middle,bottom" -TimeoutMs 5000
        Assert-SelfTest ($remoteTask.planned_commands.Count -eq 24) "remote command matrix for three states"
        Initialize-MockRemoteAdminApi
        $remoteResult = Invoke-RemoteUiAuditTask -Task $remoteTask -RunRoot $tempRoot -RunIdValue "selftest" -Backend "Mock" -BaseUrl "" -Token "" -PollIntervalMs 1
        Assert-SelfTest ($remoteResult.status -eq "passed") "mock remote single-page audit result"
        Assert-SelfTest ($remoteResult.remote_tasks.Count -eq 24) "mock remote task count"
        Assert-SelfTest ($remoteResult.captures.Count -eq 3) "mock remote captures for top middle bottom"
        Assert-SelfTest ($remoteResult.captures[0].screenshot_artifact_uri -like "artifact://debug/*/screenshot.png") "mock remote screenshot artifact URI"
        Assert-SelfTest ($remoteResult.captures[0].metadata_artifact_uri -like "artifact://debug/*/metadata.json") "mock remote metadata artifact URI"
        Assert-SelfTest ($remoteResult.captures[0].log_artifact_uri -like "artifact://debug/*/client.log") "mock remote log artifact URI"
        Assert-SelfTest ($remoteResult.captures[0].screenshot_exists -and $remoteResult.captures[0].metadata_exists -and $remoteResult.captures[0].log_exists) "mock remote artifact local mapping"

        $remoteFailureTarget = @(Resolve-RemoteUiAuditTargets -InputDeviceIds @("mock-fail-debug_disabled") -InputClientIds @() -InputSessionIds @())[0]
        $remoteFailureTask = New-RemoteUiAuditTask -RunRoot $tempRoot -Screen "ui_gallery" -RemoteTarget $remoteFailureTarget -StateValue "initial" -TimeoutMs 5000
        Initialize-MockRemoteAdminApi
        $remoteFailure = Invoke-RemoteUiAuditTask -Task $remoteFailureTask -RunRoot $tempRoot -RunIdValue "selftest" -Backend "Mock" -BaseUrl "" -Token "" -PollIntervalMs 1
        Assert-SelfTest ($remoteFailure.status -eq "failed" -and $remoteFailure.failure_type -eq "debug_disabled") "mock remote failure classification"

        $remoteEmptyArtifactTarget = @(Resolve-RemoteUiAuditTargets -InputDeviceIds @("mock-artifacts-empty") -InputClientIds @() -InputSessionIds @())[0]
        $remoteEmptyArtifactTask = New-RemoteUiAuditTask -RunRoot $tempRoot -Screen "ui_gallery" -RemoteTarget $remoteEmptyArtifactTarget -StateValue "initial" -TimeoutMs 5000
        Initialize-MockRemoteAdminApi
        $remoteEmptyArtifactFailure = Invoke-RemoteUiAuditTask -Task $remoteEmptyArtifactTask -RunRoot $tempRoot -RunIdValue "selftest" -Backend "Mock" -BaseUrl "" -Token "" -PollIntervalMs 1
        Assert-SelfTest ($remoteEmptyArtifactFailure.status -eq "failed" -and $remoteEmptyArtifactFailure.failure_type -eq "artifact_upload_failed") "mock remote empty screenshot artifacts classification"
        Assert-SelfTest ($remoteEmptyArtifactFailure.captures[0].detail -like "*screenshot*" -and $remoteEmptyArtifactFailure.captures[0].detail -like "*metadata*") "mock remote empty screenshot artifact detail"

        $remoteMissingMetadataTarget = @(Resolve-RemoteUiAuditTargets -InputDeviceIds @("mock-artifacts-missing_metadata") -InputClientIds @() -InputSessionIds @())[0]
        $remoteMissingMetadataTask = New-RemoteUiAuditTask -RunRoot $tempRoot -Screen "ui_gallery" -RemoteTarget $remoteMissingMetadataTarget -StateValue "initial" -TimeoutMs 5000
        Initialize-MockRemoteAdminApi
        $remoteMissingMetadataFailure = Invoke-RemoteUiAuditTask -Task $remoteMissingMetadataTask -RunRoot $tempRoot -RunIdValue "selftest" -Backend "Mock" -BaseUrl "" -Token "" -PollIntervalMs 1
        Assert-SelfTest ($remoteMissingMetadataFailure.status -eq "failed" -and $remoteMissingMetadataFailure.failure_type -eq "artifact_upload_failed") "mock remote missing metadata classification"
        Assert-SelfTest ($remoteMissingMetadataFailure.captures[0].screenshot_artifact_uri -like "artifact://debug/*/screenshot.png" -and [string]::IsNullOrWhiteSpace([string]$remoteMissingMetadataFailure.captures[0].metadata_artifact_uri)) "mock remote missing metadata artifact mapping"

        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "remote-selftest" -Results @($remoteResult) -ScreensValue @("ui_gallery") -DevicesValue @($remoteTargets[0].label) -IsDryRun $false -RerunSource "" -RunnerMode "Remote" -RemoteTargetsValue $remoteTargets -RemoteBackendName "Mock" -LocalDevicesValue @("desktop")
        $remoteManifest = Read-JsonFile (Join-FullPath $tempRoot "manifest.json")
        Assert-SelfTest ($remoteManifest.runner_mode -eq "remote" -and @($remoteManifest.remote_targets).Count -eq 1) "remote manifest summary"
        Assert-SelfTest (Test-Path (Join-FullPath $tempRoot "report.md")) "remote report write"

        Write-Host "Self-test passed."
    } finally {
        if (Test-Path $tempRoot) {
            Remove-Item -Recurse -Force -Path $tempRoot
        }
    }
}

function Invoke-UiAuditRunner {
    $effectiveMode = if ($Remote) { "Remote" } else { $Mode }

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

    $screensToRun = @()
    $tasks = @()

    if ($effectiveMode -eq "Remote") {
        if (-not [string]::IsNullOrWhiteSpace($RerunFromManifest)) {
            throw "Remote rerun from manifest is not supported yet. Re-run remote mode with explicit -Screens and remote target selectors."
        }

        $localFallbackDevices = @(Resolve-UiAuditDevices $Devices)
        $remoteTargets = @(Resolve-RemoteUiAuditTargets -InputDeviceIds $DeviceId -InputClientIds $ClientId -InputSessionIds $SessionId)
        $screensToRun = @(Resolve-UiAuditScreens $Screens)
        $devicesToRun = @($remoteTargets | ForEach-Object { [string]$_.label })
        $tasks = @(New-RemoteUiAuditTasks -RunRoot $runRoot -ScreensToRun $screensToRun -RemoteTargets $remoteTargets -StateValue $States -TimeoutMs $RemoteCommandTimeoutMs)

        New-Item -ItemType Directory -Force -Path $runRoot | Out-Null
        New-Item -ItemType Directory -Force -Path (Join-Path $runRoot "logs") | Out-Null
        if ($RemoteBackend -eq "Mock") {
            Initialize-MockRemoteAdminApi
        }

        Write-Host "UI audit run: $runIdValue"
        Write-Host "Mode: Remote ($RemoteBackend)"
        Write-Host "Output: $runRoot"
        Write-Host "Remote targets: $($devicesToRun -join ', ')"
        Write-Host "Local fallback devices: $($localFallbackDevices -join ', ')"
        Write-Host "Tasks: $($tasks.Count)"

        $results = New-Object System.Collections.Generic.List[object]
        if ($DryRun) {
            foreach ($task in $tasks) {
                $results.Add((New-PlannedRemoteTaskResult -Task $task -RunRoot $runRoot -RunIdValue $runIdValue))
            }
            Write-UiAuditRunnerOutputs -RunRoot $runRoot -RunIdValue $runIdValue -Results @($results.ToArray()) -ScreensValue $screensToRun -DevicesValue $devicesToRun -IsDryRun $true -RerunSource $RerunFromManifest -RunnerMode "Remote" -RemoteTargetsValue $remoteTargets -RemoteBackendName $RemoteBackend -LocalDevicesValue $localFallbackDevices
            Write-Host "Dry run complete. Remote adminapi tasks were not created."
            Write-Host "Manifest: $(Join-FullPath $runRoot "manifest.json")"
            Write-Host "Report: $(Join-FullPath $runRoot "report.md")"
            return 0
        }

        foreach ($task in $tasks) {
            Write-Host "Running remote $($task.screen) / $($task.device)"
            $result = Invoke-RemoteUiAuditTask -Task $task -RunRoot $runRoot -RunIdValue $runIdValue -Backend $RemoteBackend -BaseUrl $AdminApiBaseUrl -Token $AdminApiToken -PollIntervalMs $RemotePollIntervalMs
            $results.Add($result)
            Write-UiAuditRunnerOutputs -RunRoot $runRoot -RunIdValue $runIdValue -Results @($results.ToArray()) -ScreensValue $screensToRun -DevicesValue $devicesToRun -IsDryRun $false -RerunSource $RerunFromManifest -RunnerMode "Remote" -RemoteTargetsValue $remoteTargets -RemoteBackendName $RemoteBackend -LocalDevicesValue $localFallbackDevices

            if ($result.status -eq "passed") {
                Write-Host "  passed"
            } else {
                Write-Host "  failed: $($result.failure_type) $($result.detail)"
            }
        }

        Write-UiAuditRunnerOutputs -RunRoot $runRoot -RunIdValue $runIdValue -Results @($results.ToArray()) -ScreensValue $screensToRun -DevicesValue $devicesToRun -IsDryRun $false -RerunSource $RerunFromManifest -RunnerMode "Remote" -RemoteTargetsValue $remoteTargets -RemoteBackendName $RemoteBackend -LocalDevicesValue $localFallbackDevices
        Write-Host "Manifest: $(Join-FullPath $runRoot "manifest.json")"
        Write-Host "Report: $(Join-FullPath $runRoot "report.md")"

        $failed = @($results.ToArray() | Where-Object { $_.status -eq "failed" })
        if ($failed.Count -gt 0) {
            return 1
        }
        return 0
    }

    $extraBevyArgs = Get-WindowArgumentOverrides `
        -WindowProfileValue $WindowProfile `
        -WindowSizeValue $WindowSize `
        -DeviceScaleValue $DeviceScale `
        -WindowScaleValue $WindowScale `
        -RawBevyArgs $BevyArgs `
        -RawRemainingArgs $RemainingArgs

    $devicesToRun = @(Resolve-UiAuditDevices $Devices)
    if (-not [string]::IsNullOrWhiteSpace($RerunFromManifest)) {
        $seeds = Get-FailedTaskSeedsFromManifest -ManifestPath (Get-FullPath $RerunFromManifest) -Mode $RerunMode -MatrixDevices $devicesToRun
        if ($seeds.Count -eq 0) {
            Write-Host "No failed screen/device tasks found in $RerunFromManifest."
            return 0
        }
        $screensToRun = @($seeds | ForEach-Object { [string]$_.screen } | Select-Object -Unique)
        $devicesToRun = @($seeds | ForEach-Object { [string]$_.device } | Select-Object -Unique)
        $tasks = @(New-UiAuditTasksFromSeeds -RunRoot $runRoot -Seeds $seeds -StateValue $States -ExtraBevyArgs $extraBevyArgs)
    } else {
        $screensToRun = @(Resolve-UiAuditScreens $Screens)
        $tasks = @(New-UiAuditTasks -RunRoot $runRoot -ScreensToRun $screensToRun -DevicesToRun $devicesToRun -StateValue $States -ExtraBevyArgs $extraBevyArgs)
    }

    New-Item -ItemType Directory -Force -Path $runRoot | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $runRoot "logs") | Out-Null

    Write-Host "UI audit run: $runIdValue"
    Write-Host "Mode: Local"
    Write-Host "Output: $runRoot"
    Write-Host "Tasks: $($tasks.Count)"

    $results = New-Object System.Collections.Generic.List[object]
    if ($DryRun) {
        foreach ($task in $tasks) {
            $results.Add((New-PlannedTaskResult -Task $task -RunRoot $runRoot))
        }
        Write-UiAuditRunnerOutputs -RunRoot $runRoot -RunIdValue $runIdValue -Results @($results.ToArray()) -ScreensValue $screensToRun -DevicesValue $devicesToRun -IsDryRun $true -RerunSource $RerunFromManifest -RunnerMode "Local" -LocalDevicesValue $devicesToRun
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
        Write-UiAuditRunnerOutputs -RunRoot $runRoot -RunIdValue $runIdValue -Results @($results.ToArray()) -ScreensValue $screensToRun -DevicesValue $devicesToRun -IsDryRun $false -RerunSource $RerunFromManifest -RunnerMode "Local" -LocalDevicesValue $devicesToRun

        if ($result.status -eq "passed") {
            Write-Host "  passed"
        } else {
            Write-Host "  failed: $($result.failure_type) $($result.detail)"
        }
    }

    Write-UiAuditRunnerOutputs -RunRoot $runRoot -RunIdValue $runIdValue -Results @($results.ToArray()) -ScreensValue $screensToRun -DevicesValue $devicesToRun -IsDryRun $false -RerunSource $RerunFromManifest -RunnerMode "Local" -LocalDevicesValue $devicesToRun
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
