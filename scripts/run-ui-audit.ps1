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
    [switch]$DeterministicCapture,
    [string]$Locale = "zh_cn",
    [ValidateSet("default")]
    [string]$Theme = "default",
    [UInt64]$RandomSeed = 0,
    [ValidateScript({
        if ([double]::IsNaN($_) -or [double]::IsInfinity($_) -or $_ -lt 0) {
            throw "FrozenTimeSeconds must be a finite, non-negative number."
        }
        $true
    })]
    [double]$FrozenTimeSeconds = 0,
    [ValidateSet("StableFixture", "ExplicitMask")]
    [string]$DynamicContentPolicy = "StableFixture",
    [string]$StableFixtureId = "repository_static_data",
    [string]$DynamicMaskId = "",
    [ValidateRange(2, 8)]
    [int]$RepeatCaptures = 2,
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
    [ValidateSet("Auto", "Fixture", "Provider", "Off")]
    [string]$AnalysisMode = "Auto",
    [string]$AnalysisResultPath = "",
    [string]$ReferenceManifest = "",
    [switch]$StrictReference,
    [ValidateSet("Fixture", "Off", "Provider")]
    [string]$ComparisonAiMode = "Fixture",
    [int]$ComparisonBudgetSeconds = 1800,
    [Int64]$ComparisonBudgetPeakMemoryBytes = 805306368,
    [Int64]$ComparisonBudgetArtifactBytes = 1073741824,
    [switch]$RequireRealAndroid,
    [ValidateSet("Off", "Plan", "Mock", "Command")]
    [string]$FixMode = "Off",
    [int]$MaxFixIterations = 5,
    [string]$FixCommand = "",
    [ValidateSet("Pass", "MaxIterations", "CheckFailed", "UnsafePath")]
    [string]$MockFixScenario = "Pass",
    [ValidateSet("Off", "Fixture", "Plan", "Provider")]
    [string]$GenerationMode = "Off",
    [string]$GenerationTask = "",
    [string]$GenerationOptions = "",
    [string]$GenerationDocumentId = "generated.audit_draft",
    [string]$ProviderCredentialEnvironment = "",
    [switch]$DryRun,
    [switch]$SelfTest,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$RemainingArgs = @()
)

$ErrorActionPreference = "Stop"
$script:MaxUiAuditDeterministicHardFailures = 1024

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
    [pscustomobject]@{ Canonical = "ui_document_gallery"; Aliases = @("ui_document_gallery", "ui-document-gallery", "document_gallery", "document-gallery", "declarative_gallery") },
    [pscustomobject]@{ Canonical = "ui_generated_acceptance"; Aliases = @("ui_generated_acceptance", "ui-generated-acceptance", "generated_acceptance") },
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

$script:AnalysisSeverityLevels = @("severe", "medium", "minor")
$script:AnalysisBlockingProblemTypes = @(
    "text_overlap",
    "critical_clipping",
    "unclickable",
    "critical_content_unreachable",
    "modal_layering_error"
)
$script:LastUiAuditAnalysisStatus = $null
$script:UiAuditClosedLoopGeneration = $null
$script:UiAuditComparisonToolManifest = "tools/ui-visual-audit/Cargo.toml"
$script:UiAuditReferenceManifestSchemaVersion = 1

$script:FixStrategyPriority = @(
    [ordered]@{
        id = "page_local_layout"
        label = "page local layout"
        allowed_roots = @("project/src/game/screens/")
        description = "Prefer screen-owned layout and spacing changes before shared code."
    },
    [ordered]@{
        id = "common_widgets"
        label = "common widgets"
        allowed_roots = @("project/src/framework/ui/widgets/")
        description = "Change shared UI widgets only when a page-local fix cannot address the issue."
    },
    [ordered]@{
        id = "theme_tokens"
        label = "theme tokens"
        allowed_roots = @("project/src/framework/ui/style/")
        description = "Adjust theme tokens after checking page and widget scopes."
    },
    [ordered]@{
        id = "framework_core"
        label = "framework core"
        allowed_roots = @("project/src/framework/ui/core/", "project/src/framework/ui/overlays/")
        description = "Use framework-level changes as the last resort."
    }
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
            return "image_fit,visual_foundation,visual_acceptance,image_modes,image_tiling,image_atlas,typography,typography_overflow,icons,icon_states,style_scopes,effects,animations,components,component_checkboxes,component_toggles,component_segmented,component_overlays,component_tooltip,middle,bottom"
        }
        return "initial"
    }

    $valid = @("initial", "visual_foundation", "visual_acceptance", "image_fit", "image_modes", "image_tiling", "image_atlas", "typography", "typography_overflow", "icons", "icon_states", "style_scopes", "effects", "animations", "components", "component_checkboxes", "component_toggles", "component_segmented", "component_overlays", "component_tooltip", "top", "middle", "bottom")
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

function Get-UiAuditDeterministicProfile {
    param([Parameter(Mandatory = $true)][string]$Device)

    switch ($Device) {
        "desktop" { return [pscustomobject]@{ logical_width = 1280.0; logical_height = 720.0; physical_width = 1280; physical_height = 720; device_scale = 1.0 } }
        "phone-portrait" { return [pscustomobject]@{ logical_width = (1280.0 / 3.25); logical_height = (2772.0 / 3.25); physical_width = 1280; physical_height = 2772; device_scale = 3.25 } }
        "phone-1080p" { return [pscustomobject]@{ logical_width = 360.0; logical_height = 800.0; physical_width = 1080; physical_height = 2400; device_scale = 3.0 } }
        "phone-small" { return [pscustomobject]@{ logical_width = 360.0; logical_height = 800.0; physical_width = 720; physical_height = 1600; device_scale = 2.0 } }
        "tablet-portrait" { return [pscustomobject]@{ logical_width = 800.0; logical_height = 1280.0; physical_width = 1600; physical_height = 2560; device_scale = 2.0 } }
        "tablet-landscape" { return [pscustomobject]@{ logical_width = 1280.0; logical_height = 800.0; physical_width = 2560; physical_height = 1600; device_scale = 2.0 } }
        default { throw "Deterministic capture does not know window profile '$Device'." }
    }
}

function Get-UiAuditGitCommit {
    param([Parameter(Mandatory = $true)][string]$RepositoryRoot)

    try {
        $commit = (& git -C $RepositoryRoot rev-parse HEAD 2>$null | Select-Object -First 1)
        if (-not [string]::IsNullOrWhiteSpace([string]$commit) -and ([string]$commit).Trim() -match '^[0-9a-fA-F]{7,64}$') {
            return ([string]$commit).Trim().ToLowerInvariant()
        }
    } catch {
    }
    return "unknown"
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

function New-UiAuditArtifactLink {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $full = Get-FullPath $Path
    if (-not (Test-Path -LiteralPath $full -PathType Leaf)) {
        throw "Artifact link target is missing: $full"
    }
    $item = Get-Item -LiteralPath $full
    return [ordered]@{
        path = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $full
        sha256 = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()
        byte_length = [int64]$item.Length
    }
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
    $determinism = $null
    if ($DeterministicCapture) {
        if ([string]::IsNullOrWhiteSpace($Locale) -or [string]::IsNullOrWhiteSpace($Theme)) {
            throw "Deterministic capture requires non-empty -Locale and -Theme."
        }
        if ($DynamicContentPolicy -eq "StableFixture" -and [string]::IsNullOrWhiteSpace($StableFixtureId)) {
            throw "StableFixture dynamic policy requires -StableFixtureId."
        }
        if ($DynamicContentPolicy -eq "ExplicitMask" -and [string]::IsNullOrWhiteSpace($DynamicMaskId)) {
            throw "ExplicitMask dynamic policy requires -DynamicMaskId."
        }
        $determinism = [pscustomobject]@{
            target_viewport = Get-UiAuditDeterministicProfile -Device $Device
            locale = $Locale.Trim().ToLowerInvariant().Replace("-", "_")
            theme = $Theme.Trim()
            random_seed = $RandomSeed
            frozen_time_seconds = $FrozenTimeSeconds
            animation_progress = 1.0
            dynamic_policy = if ($DynamicContentPolicy -eq "StableFixture") { "stable_fixture" } else { "explicit_mask" }
            stable_fixture_id = if ($DynamicContentPolicy -eq "StableFixture") { $StableFixtureId.Trim() } else { $null }
            dynamic_mask_id = if ($DynamicContentPolicy -eq "ExplicitMask") { $DynamicMaskId.Trim() } else { $null }
            repeat_captures = $RepeatCaptures
        }
    }

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
        determinism = $determinism
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

function Get-UiAuditLikelyFiles {
    param([Parameter(Mandatory = $true)][string]$Screen)

    $common = @(
        "project/src/framework/ui/widgets/controls.rs",
        "project/src/framework/ui/widgets/layout.rs",
        "project/src/framework/ui/widgets/scroll.rs",
        "project/src/framework/ui/style/theme.rs",
        "project/src/framework/ui/audit/local.rs"
    )

    switch ($Screen) {
        "ui_gallery" {
            return @(
                "project/src/game/screens/dev/ui_gallery.rs",
                "project/src/game/screens/dev/mod.rs",
                "project/src/game/navigation/mod.rs"
            ) + $common
        }
        "ui_document_gallery" {
            return @(
                "project/src/game/screens/dev/ui_document_gallery.rs",
                "project/assets/ui/documents/approved/gallery/declarative_gallery.v1.json",
                "project/src/framework/ui/document/preview.rs",
                "project/src/framework/ui/document/runtime.rs",
                "project/src/game/screens/dev/mod.rs",
                "project/src/game/navigation/mod.rs"
            ) + $common
        }
        "login" {
            return @(
                "project/src/game/screens/auth/login.rs",
                "project/src/game/screens/auth/mod.rs",
                "project/src/game/navigation/mod.rs"
            ) + $common
        }
        "lobby" {
            return @(
                "project/src/game/screens/lobby/mod.rs",
                "project/src/game/screens/lobby/game_list.rs",
                "project/src/game/navigation/mod.rs"
            ) + $common
        }
        "audio_settings" {
            return @(
                "project/src/game/screens/settings/audio.rs",
                "project/src/game/screens/settings/mod.rs",
                "project/src/game/navigation/mod.rs"
            ) + $common
        }
        "audio_monitor" {
            return @(
                "project/src/game/screens/dev/audio_monitor.rs",
                "project/src/game/screens/dev/mod.rs",
                "project/src/game/navigation/mod.rs"
            ) + $common
        }
        "audio_gallery" {
            return @(
                "project/src/game/screens/dev/audio_gallery.rs",
                "project/src/game/screens/dev/mod.rs",
                "project/src/game/navigation/mod.rs"
            ) + $common
        }
        "wanfa_touch_ripple" {
            return @(
                "project/src/game/screens/gameplay/touch_ripple.rs",
                "project/src/game/features/touch_ripple/visual.rs",
                "project/src/game/navigation/mod.rs"
            ) + $common
        }
        "sample_scene" {
            return @(
                "project/src/game/screens/gameplay/sample_scene.rs",
                "project/src/game/scenes/mod.rs",
                "project/src/game/navigation/mod.rs"
            ) + $common
        }
        "robot_sync_scene" {
            return @(
                "project/src/game/screens/gameplay/robot_sync_scene.rs",
                "project/src/game/scenes/mod.rs",
                "project/src/game/navigation/mod.rs"
            ) + $common
        }
        "fangyuan_home" {
            return @(
                "project/src/game/screens/gameplay/fangyuan_home.rs",
                "project/src/game/navigation/mod.rs"
            ) + $common
        }
        default {
            return @(
                "project/src/game/screens/mod.rs",
                "project/src/game/navigation/mod.rs"
            ) + $common
        }
    }
}

function New-UiAuditAnalysisInput {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)]$Manifest
    )

    $captures = New-Object System.Collections.Generic.List[object]
    foreach ($task in @($Manifest.tasks)) {
        foreach ($capture in @($task.captures)) {
            if ($null -eq $capture) {
                continue
            }

            $screen = [string]$capture.screen
            if ([string]::IsNullOrWhiteSpace($screen)) {
                $screen = [string]$task.screen
            }
            $device = [string]$capture.device
            if ([string]::IsNullOrWhiteSpace($device)) {
                $device = [string]$task.device
            }
            $state = [string]$capture.state
            $remoteTaskIds = @()
            if ($null -ne $capture.PSObject.Properties["remote_task_ids"]) {
                $remoteTaskIds = @($capture.remote_task_ids | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) } | ForEach-Object { [string]$_ })
            }

            $captures.Add([pscustomobject]@{
                capture_id = "$screen.$device.$state"
                screen = $screen
                device = $device
                state = $state
                status = [string]$capture.status
                screenshot = if ($null -ne $capture.PSObject.Properties["screenshot"]) { [string]$capture.screenshot } else { $null }
                metadata = if ($null -ne $capture.PSObject.Properties["metadata"]) { [string]$capture.metadata } else { $null }
                screenshot_artifact_uri = if ($null -ne $capture.PSObject.Properties["screenshot_artifact_uri"]) { [string]$capture.screenshot_artifact_uri } else { $null }
                metadata_artifact_uri = if ($null -ne $capture.PSObject.Properties["metadata_artifact_uri"]) { [string]$capture.metadata_artifact_uri } else { $null }
                screenshot_exists = if ($null -ne $capture.PSObject.Properties["screenshot_exists"]) { [bool]$capture.screenshot_exists } else { $false }
                metadata_exists = if ($null -ne $capture.PSObject.Properties["metadata_exists"]) { [bool]$capture.metadata_exists } else { $false }
                manifest = "manifest.json"
                likely_files = @(Get-UiAuditLikelyFiles -Screen $screen)
                remote_task_ids = $remoteTaskIds
                screenshot_task_id = if ($null -ne $capture.PSObject.Properties["screenshot_task_id"]) { [string]$capture.screenshot_task_id } else { $null }
                metadata_task_id = if ($null -ne $capture.PSObject.Properties["metadata_task_id"]) { [string]$capture.metadata_task_id } else { $null }
            })
        }
    }

    return [pscustomobject]@{
        schema_version = 1
        run_id = [string]$Manifest.run_id
        runner_mode = [string]$Manifest.runner_mode
        manifest = "manifest.json"
        created_at = (Get-Date).ToString("o")
        captures = @($captures.ToArray())
    }
}

function Write-UiAuditAnalysisInput {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)]$AnalysisInput
    )

    $analysisInputPath = Join-FullPath $RunRoot "analysis-input.json"
    $AnalysisInput | ConvertTo-Json -Depth 20 | Set-Content -Path $analysisInputPath -Encoding UTF8
    return ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $analysisInputPath
}

function Test-UiAuditCaptureAnalysisReady {
    param(
        [Parameter(Mandatory = $true)][object]$Capture,
        [Parameter(Mandatory = $true)][bool]$IsRemote
    )

    if ($IsRemote) {
        return (
            -not [string]::IsNullOrWhiteSpace([string]$Capture.screenshot_artifact_uri) -and
            -not [string]::IsNullOrWhiteSpace([string]$Capture.metadata_artifact_uri)
        )
    }

    return (
        -not [string]::IsNullOrWhiteSpace([string]$Capture.screenshot) -and
        -not [string]::IsNullOrWhiteSpace([string]$Capture.metadata) -and
        [bool]$Capture.screenshot_exists -and
        [bool]$Capture.metadata_exists
    )
}

function Get-UiAuditAnalysisInputFailureType {
    param(
        [Parameter(Mandatory = $true)]$AnalysisInput,
        [Parameter(Mandatory = $true)][bool]$IsRemote
    )

    $captures = @($AnalysisInput.captures)
    if ($captures.Count -eq 0) {
        return "ai_missing_capture"
    }

    foreach ($capture in $captures) {
        if ($IsRemote) {
            if ([string]::IsNullOrWhiteSpace([string]$capture.screenshot_artifact_uri) -or [string]::IsNullOrWhiteSpace([string]$capture.metadata_artifact_uri)) {
                return "ai_remote_artifact_read_failed"
            }
        } elseif (-not (Test-UiAuditCaptureAnalysisReady -Capture $capture -IsRemote $false)) {
            return "ai_missing_capture_metadata"
        }
    }

    return $null
}

function Get-UiAuditIssueKey {
    param([Parameter(Mandatory = $true)]$Issue)

    return "$($Issue.screen)|$($Issue.device)|$($Issue.state)"
}

function ConvertTo-UiAuditIssueSeverity {
    param(
        [AllowNull()][string]$Severity,
        [AllowNull()][string]$ProblemType,
        [AllowNull()][string]$Problem
    )

    $normalizedSeverity = if ([string]::IsNullOrWhiteSpace($Severity)) { "" } else { $Severity.Trim().ToLowerInvariant() }
    $type = if ([string]::IsNullOrWhiteSpace($ProblemType)) { "" } else { $ProblemType.Trim().ToLowerInvariant().Replace("-", "_") }
    $text = if ([string]::IsNullOrWhiteSpace($Problem)) { "" } else { $Problem.Trim().ToLowerInvariant() }

    $inferredSeverity = "minor"

    if ($type -in $script:AnalysisBlockingProblemTypes) {
        $inferredSeverity = "severe"
    } elseif ($type -in @("small_text", "crowded_spacing", "unstable_list_item_height", "small_touch_target", "visual_hierarchy_confusing")) {
        $inferredSeverity = "medium"
    } elseif ($text -match "文字重叠|重叠|overlap|关键裁切|裁切|clipping|不可点击|unclickable|关键内容不可达|不可达|unreachable|弹窗层级错误|层级错误|modal.*layer|关键内容跑出屏幕|out of screen") {
        $inferredSeverity = "severe"
    } elseif ($text -match "文本过小|不可读|too small|unreadable|间距.*拥挤|拥挤|crowded|触控目标.*小|touch target|主次层级混乱|hierarchy|列表项高度") {
        $inferredSeverity = "medium"
    }

    if ($normalizedSeverity -notin $script:AnalysisSeverityLevels) {
        return $inferredSeverity
    }

    $rank = @{ minor = 1; medium = 2; severe = 3 }
    if ($rank[$inferredSeverity] -gt $rank[$normalizedSeverity]) {
        return $inferredSeverity
    }

    return $normalizedSeverity
}

function Test-UiAuditIssueBlocking {
    param(
        [Parameter(Mandatory = $true)][string]$Severity,
        [AllowNull()][object]$Blocking,
        [AllowNull()][string]$ProblemType,
        [AllowNull()][string]$Problem
    )

    if ($Severity -in @("severe", "medium")) {
        return $true
    }

    if ($null -ne $Blocking -and "$Blocking" -match "^(?i:true|false)$") {
        return [System.Convert]::ToBoolean([string]$Blocking)
    }

    $classified = ConvertTo-UiAuditIssueSeverity -Severity "" -ProblemType $ProblemType -Problem $Problem
    return ($classified -in @("severe", "medium"))
}

function Assert-UiAuditIssueRequiredFields {
    param(
        [Parameter(Mandatory = $true)]$Issue,
        [Parameter(Mandatory = $true)][int]$Index
    )

    foreach ($field in @("screen", "device", "state", "problem", "evidence", "likely_cause", "suggested_files")) {
        if ($null -eq $Issue.PSObject.Properties[$field]) {
            throw "issue[$Index] is missing required field '$field'"
        }
    }
    if ([string]::IsNullOrWhiteSpace([string]$Issue.screen) -or
        [string]::IsNullOrWhiteSpace([string]$Issue.device) -or
        [string]::IsNullOrWhiteSpace([string]$Issue.state) -or
        [string]::IsNullOrWhiteSpace([string]$Issue.problem) -or
        [string]::IsNullOrWhiteSpace([string]$Issue.evidence) -or
        [string]::IsNullOrWhiteSpace([string]$Issue.likely_cause)) {
        throw "issue[$Index] has blank required fields"
    }
}

function ConvertTo-UiAuditAnalysisIssues {
    param(
        [Parameter(Mandatory = $true)]$RawAnalysis,
        [Parameter(Mandatory = $true)]$AnalysisInput
    )

    if ($null -eq $RawAnalysis.PSObject.Properties["issues"]) {
        throw "analysis result is missing required field 'issues'"
    }

    $validCaptureKeys = New-Object 'System.Collections.Generic.HashSet[string]'
    foreach ($capture in @($AnalysisInput.captures)) {
        [void]$validCaptureKeys.Add("$($capture.screen)|$($capture.device)|$($capture.state)")
    }

    $issues = New-Object System.Collections.Generic.List[object]
    $index = 0
    foreach ($issue in @($RawAnalysis.issues)) {
        $normalizedInputIssue = $issue
        if ($null -ne $issue.PSObject.Properties["capture_id"] -and $null -eq $issue.PSObject.Properties["screen"]) {
            $captureId = [string]$issue.capture_id
            $matchedCaptures = @($AnalysisInput.captures | Where-Object { [string]$_.capture_id -eq $captureId })
            if ($matchedCaptures.Count -ne 1) {
                throw "issue[$Index] does not match exactly one analysis input capture_id: $captureId"
            }
            $capture = $matchedCaptures[0]
            $evidenceDescriptions = New-Object System.Collections.Generic.List[string]
            foreach ($entry in @($issue.evidence)) {
                if ($null -eq $entry.PSObject.Properties["image_id"] -or $null -eq $entry.PSObject.Properties["description"]) {
                    throw "issue[$Index] structured evidence is missing image_id or description"
                }
                $imageId = [string]$entry.image_id
                if (-not $imageId.StartsWith("$captureId.", [System.StringComparison]::Ordinal)) {
                    throw "issue[$Index] structured evidence does not belong to capture_id: $imageId"
                }
                $evidenceDescriptions.Add([string]$entry.description)
            }
            if ($evidenceDescriptions.Count -eq 0) {
                throw "issue[$Index] structured evidence is empty"
            }
            $normalizedInputIssue = [pscustomobject]@{
                screen = [string]$capture.screen
                device = [string]$capture.device
                state = [string]$capture.state
                severity = if ($null -ne $issue.PSObject.Properties["severity"]) { [string]$issue.severity } else { "" }
                problem_type = if ($null -ne $issue.PSObject.Properties["problem_type"]) { [string]$issue.problem_type } else { $null }
                problem = [string]$issue.problem
                evidence = ($evidenceDescriptions.ToArray() -join "; ")
                likely_cause = [string]$issue.likely_cause
                suggested_files = @($issue.suggested_files)
                blocking = $null
                capture_id = $captureId
                evidence_images = @($issue.evidence | ForEach-Object { [string]$_.image_id })
                region = if ($null -ne $issue.PSObject.Properties["region"]) { $issue.region } else { $null }
                reference_element = if ($null -ne $issue.PSObject.Properties["reference_element"]) { $issue.reference_element } else { $null }
                node_id = if ($null -ne $issue.PSObject.Properties["node_id"]) { $issue.node_id } else { $null }
            }
        }

        Assert-UiAuditIssueRequiredFields -Issue $normalizedInputIssue -Index $index

        $severity = ConvertTo-UiAuditIssueSeverity `
            -Severity $(if ($null -ne $normalizedInputIssue.PSObject.Properties["severity"]) { [string]$normalizedInputIssue.severity } else { "" }) `
            -ProblemType $(if ($null -ne $normalizedInputIssue.PSObject.Properties["problem_type"]) { [string]$normalizedInputIssue.problem_type } else { "" }) `
            -Problem ([string]$normalizedInputIssue.problem)
        $problemType = if ($null -ne $normalizedInputIssue.PSObject.Properties["problem_type"]) { [string]$normalizedInputIssue.problem_type } else { $null }
        $blocking = Test-UiAuditIssueBlocking `
            -Severity $severity `
            -Blocking $(if ($null -ne $normalizedInputIssue.PSObject.Properties["blocking"]) { $normalizedInputIssue.blocking } else { $null }) `
            -ProblemType $problemType `
            -Problem ([string]$normalizedInputIssue.problem)

        $normalized = [pscustomobject]@{
            screen = [string]$normalizedInputIssue.screen
            device = [string]$normalizedInputIssue.device
            state = [string]$normalizedInputIssue.state
            severity = $severity
            problem_type = $problemType
            problem = [string]$normalizedInputIssue.problem
            evidence = [string]$normalizedInputIssue.evidence
            likely_cause = [string]$normalizedInputIssue.likely_cause
            suggested_files = @($normalizedInputIssue.suggested_files | ForEach-Object { [string]$_ })
            blocking = [bool]$blocking
            capture_id = if ($null -ne $normalizedInputIssue.PSObject.Properties["capture_id"]) { [string]$normalizedInputIssue.capture_id } else { $null }
            evidence_images = if ($null -ne $normalizedInputIssue.PSObject.Properties["evidence_images"]) { @($normalizedInputIssue.evidence_images) } else { @() }
            region = if ($null -ne $normalizedInputIssue.PSObject.Properties["region"]) { $normalizedInputIssue.region } else { $null }
            reference_element = if ($null -ne $normalizedInputIssue.PSObject.Properties["reference_element"]) { $normalizedInputIssue.reference_element } else { $null }
            node_id = if ($null -ne $normalizedInputIssue.PSObject.Properties["node_id"]) { $normalizedInputIssue.node_id } else { $null }
        }

        if (-not $validCaptureKeys.Contains((Get-UiAuditIssueKey -Issue $normalized))) {
            throw "issue[$Index] does not match any analysis input capture: $($normalized.screen)/$($normalized.device)/$($normalized.state)"
        }

        $issues.Add($normalized)
        $index += 1
    }

    return @($issues.ToArray())
}

function Assert-UiAuditProviderReport {
    param(
        [Parameter(Mandatory = $true)]$Report,
        [Parameter(Mandatory = $true)]$AnalysisInput
    )

    foreach ($field in @("schema_version", "algorithm_version", "status", "provider", "input", "issues", "deterministic_hard_failures", "deterministic_hard_failures_preserved", "visual_similarity_is_sole_conclusion", "privacy", "artifacts")) {
        if ($null -eq $Report.PSObject.Properties[$field]) {
            throw "provider analysis report is missing required field '$field'"
        }
    }
    if ([int]$Report.schema_version -ne 1 -or
        [string]$Report.algorithm_version -ne "ui_ai_visual_analysis_v1" -or
        [string]$Report.status -ne "completed" -or
        -not [bool]$Report.deterministic_hard_failures_preserved -or
        [bool]$Report.visual_similarity_is_sole_conclusion) {
        throw "provider analysis report violates the version, status, or deterministic-preservation contract"
    }
    foreach ($field in @("mode", "provider_id", "audit_model_id", "self_review_is_sole_conclusion", "attempts")) {
        if ($null -eq $Report.provider.PSObject.Properties[$field]) {
            throw "provider analysis report provider is missing '$field'"
        }
    }
    if ([bool]$Report.provider.self_review_is_sole_conclusion -or
        [string]::IsNullOrWhiteSpace([string]$Report.provider.provider_id) -or
        [string]::IsNullOrWhiteSpace([string]$Report.provider.audit_model_id)) {
        throw "provider analysis report has unsafe provider/model evidence"
    }
    foreach ($field in @("bundle_path", "bundle_sha256", "capture_count", "image_count", "image_bytes", "region_metric_count", "semantic_node_count", "provider_images")) {
        if ($null -eq $Report.input.PSObject.Properties[$field]) {
            throw "provider analysis report input is missing '$field'"
        }
    }
    if (@($Report.input.provider_images).Count -ne [int]$Report.input.image_count) {
        throw "provider analysis report image provenance count does not match image_count"
    }
    foreach ($image in @($Report.input.provider_images)) {
        foreach ($field in @("image_id", "source_sha256", "provider_sha256", "redaction_rect_count")) {
            if ($null -eq $image.PSObject.Properties[$field]) {
                throw "provider analysis report image provenance is missing '$field'"
            }
        }
        if ([string]$image.source_sha256 -notmatch "^[0-9a-f]{64}$" -or [string]$image.provider_sha256 -notmatch "^[0-9a-f]{64}$") {
            throw "provider analysis report image provenance hash is invalid"
        }
    }
    foreach ($field in @("credentials_persisted", "image_bytes_persisted", "raw_provider_response_persisted", "prompt_persisted", "sensitive_text_redaction", "provider_redacted_image_count", "provider_redaction_rect_count", "metadata_sensitive_string_count", "response_redaction_count")) {
        if ($null -eq $Report.privacy.PSObject.Properties[$field]) {
            throw "provider analysis report privacy is missing '$field'"
        }
    }
    if ([bool]$Report.privacy.credentials_persisted -or
        [bool]$Report.privacy.image_bytes_persisted -or
        [bool]$Report.privacy.raw_provider_response_persisted -or
        [bool]$Report.privacy.prompt_persisted) {
        throw "provider analysis report indicates persisted sensitive provider payloads"
    }

    $hardFailures = @($Report.deterministic_hard_failures)
    if ($hardFailures.Count -gt $script:MaxUiAuditDeterministicHardFailures) {
        throw "provider analysis report deterministic hard failure count exceeds the fixed limit"
    }
    $captureCounts = @{}
    foreach ($capture in @($AnalysisInput.captures)) {
        if ($null -eq $capture.PSObject.Properties["capture_id"] -or
            [string]::IsNullOrWhiteSpace([string]$capture.capture_id)) {
            throw "analysis input capture is missing its explicit capture_id"
        }
        $captureId = [string]$capture.capture_id
        if ($captureCounts.ContainsKey($captureId)) {
            $captureCounts[$captureId] += 1
        } else {
            $captureCounts[$captureId] = 1
        }
    }
    $hardFailureIndex = 0
    foreach ($hardFailure in $hardFailures) {
        foreach ($field in @("capture_id", "finding")) {
            if ($null -eq $hardFailure.PSObject.Properties[$field]) {
                throw "provider deterministic hard failure[$hardFailureIndex] is missing '$field'"
            }
        }
        $captureId = [string]$hardFailure.capture_id
        if ([string]::IsNullOrWhiteSpace($captureId) -or
            -not $captureCounts.ContainsKey($captureId) -or
            [int]$captureCounts[$captureId] -ne 1) {
            throw "provider deterministic hard failure[$hardFailureIndex] does not bind exactly one analysis input capture"
        }
        foreach ($field in @("code", "severity", "message", "primary")) {
            if ($null -eq $hardFailure.finding.PSObject.Properties[$field] -or
                $null -eq $hardFailure.finding.$field) {
                throw "provider deterministic hard failure[$hardFailureIndex] finding is missing '$field'"
            }
        }
        if ([string]::IsNullOrWhiteSpace([string]$hardFailure.finding.code) -or
            [string]::IsNullOrWhiteSpace([string]$hardFailure.finding.severity) -or
            [string]::IsNullOrWhiteSpace([string]$hardFailure.finding.message)) {
            throw "provider deterministic hard failure[$hardFailureIndex] finding has empty required evidence"
        }
        $hardFailureIndex += 1
    }
}

function New-UiAuditAnalysisSummary {
    param([AllowEmptyCollection()][object[]]$Issues)

    $severe = @($Issues | Where-Object { $_.severity -eq "severe" })
    $medium = @($Issues | Where-Object { $_.severity -eq "medium" })
    $minor = @($Issues | Where-Object { $_.severity -eq "minor" })
    $blocking = @($Issues | Where-Object { [bool]$_.blocking })

    return [ordered]@{
        total = $Issues.Count
        severe = $severe.Count
        medium = $medium.Count
        minor = $minor.Count
        blocking = $blocking.Count
    }
}

function New-UiAuditAnalysisFailure {
    param(
        [Parameter(Mandatory = $true)][string]$Mode,
        [Parameter(Mandatory = $true)][string]$FailureType,
        [Parameter(Mandatory = $true)][string]$Detail,
        [Parameter(Mandatory = $true)]$AnalysisInput,
        [AllowNull()][string]$InputPath,
        [AllowNull()][string]$ResultPath
    )

    return [pscustomobject]@{
        schema_version = 1
        mode = $Mode
        status = "failed"
        pass = $false
        failure_type = $FailureType
        detail = $Detail
        input = [ordered]@{
            path = $InputPath
            capture_count = @($AnalysisInput.captures).Count
            runner_mode = [string]$AnalysisInput.runner_mode
        }
        result_path = $ResultPath
        severity_counts = [ordered]@{ total = 0; severe = 0; medium = 0; minor = 0; blocking = 0 }
        issues = @()
    }
}

function Invoke-UiAuditAnalysis {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)]$AnalysisInput,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$InputPath,
        [Parameter(Mandatory = $true)][string]$Mode,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$ResultPath
    )

    $normalizedMode = $Mode.Trim()
    if ($normalizedMode -eq "Off") {
        return [pscustomobject]@{
            schema_version = 1
            mode = "Off"
            status = "skipped"
            pass = $true
            failure_type = $null
            detail = "analysis disabled"
            input = [ordered]@{
                path = $InputPath
                capture_count = @($AnalysisInput.captures).Count
                runner_mode = [string]$AnalysisInput.runner_mode
            }
            result_path = $null
            severity_counts = [ordered]@{ total = 0; severe = 0; medium = 0; minor = 0; blocking = 0 }
            issues = @()
        }
    }

    $shouldReadFixture = $normalizedMode -in @("Fixture", "Provider") -or -not [string]::IsNullOrWhiteSpace($ResultPath)
    if (-not $shouldReadFixture) {
        return [pscustomobject]@{
            schema_version = 1
            mode = $normalizedMode
            status = "skipped"
            pass = $true
            failure_type = $null
            detail = "no analysis result supplied; run the explicit analyze-ai adapter or provide a fixture result"
            input = [ordered]@{
                path = $InputPath
                capture_count = @($AnalysisInput.captures).Count
                runner_mode = [string]$AnalysisInput.runner_mode
            }
            result_path = $null
            severity_counts = [ordered]@{ total = 0; severe = 0; medium = 0; minor = 0; blocking = 0 }
            issues = @()
        }
    }

    $isRemote = ([string]$AnalysisInput.runner_mode) -eq "remote"
    $inputFailure = Get-UiAuditAnalysisInputFailureType -AnalysisInput $AnalysisInput -IsRemote $isRemote
    if (-not [string]::IsNullOrWhiteSpace($inputFailure)) {
        return New-UiAuditAnalysisFailure -Mode $normalizedMode -FailureType $inputFailure -Detail "analysis input is missing required screenshot or metadata references" -AnalysisInput $AnalysisInput -InputPath $InputPath -ResultPath $ResultPath
    }

    if ([string]::IsNullOrWhiteSpace($ResultPath) -or -not (Test-Path $ResultPath)) {
        return New-UiAuditAnalysisFailure -Mode $normalizedMode -FailureType "ai_analysis_failed" -Detail "analysis result fixture was not found" -AnalysisInput $AnalysisInput -InputPath $InputPath -ResultPath $ResultPath
    }

    try {
        $raw = Read-JsonFile (Get-FullPath $ResultPath)
    } catch {
        return New-UiAuditAnalysisFailure -Mode $normalizedMode -FailureType "ai_result_invalid" -Detail $_.Exception.Message -AnalysisInput $AnalysisInput -InputPath $InputPath -ResultPath $ResultPath
    }

    if ($normalizedMode -eq "Provider") {
        try {
            Assert-UiAuditProviderReport -Report $raw -AnalysisInput $AnalysisInput
        } catch {
            return New-UiAuditAnalysisFailure -Mode $normalizedMode -FailureType "ai_result_invalid" -Detail $_.Exception.Message -AnalysisInput $AnalysisInput -InputPath $InputPath -ResultPath $ResultPath
        }
    }

    try {
        $issues = @(ConvertTo-UiAuditAnalysisIssues -RawAnalysis $raw -AnalysisInput $AnalysisInput)
    } catch {
        return New-UiAuditAnalysisFailure -Mode $normalizedMode -FailureType "ai_result_invalid" -Detail $_.Exception.Message -AnalysisInput $AnalysisInput -InputPath $InputPath -ResultPath $ResultPath
    }

    $counts = New-UiAuditAnalysisSummary -Issues $issues
    $blockingIssues = @($issues | Where-Object { [bool]$_.blocking -or $_.severity -in @("severe", "medium") })
    $deterministicHardFailures = if ($normalizedMode -eq "Provider") { @($raw.deterministic_hard_failures) } else { @() }
    $deterministicHardFailureCount = $deterministicHardFailures.Count
    $status = if ($deterministicHardFailureCount -gt 0 -or $blockingIssues.Count -gt 0) { "failed" } else { "passed" }
    $failureType = if ($deterministicHardFailureCount -gt 0) {
        "deterministic_hard_failure"
    } elseif ($blockingIssues.Count -gt 0) {
        "ai_blocking_issue"
    } else {
        $null
    }

    $analysisResult = [pscustomobject]@{
        schema_version = 1
        mode = $normalizedMode
        status = $status
        pass = ($status -eq "passed")
        failure_type = $failureType
        detail = if ($failureType -eq "deterministic_hard_failure") {
            "deterministic semantic hard failure found"
        } elseif ($failureType) {
            "severe, medium, or blocking analysis issue found"
        } else { $null }
        input = [ordered]@{
            path = $InputPath
            capture_count = @($AnalysisInput.captures).Count
            runner_mode = [string]$AnalysisInput.runner_mode
        }
        result_path = if ([string]::IsNullOrWhiteSpace($ResultPath)) { $null } else { $ResultPath }
        severity_counts = $counts
        issues = @($issues)
    }
    if ($normalizedMode -eq "Provider") {
        $analysisResult | Add-Member -NotePropertyName deterministic_hard_failure_count -NotePropertyValue $deterministicHardFailureCount
        $analysisResult | Add-Member -NotePropertyName deterministic_hard_failures -NotePropertyValue @($deterministicHardFailures)
    }
    return $analysisResult
}

function Write-UiAuditAnalysisOutput {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)]$Analysis,
        [Parameter(Mandatory = $true)][string]$RunIdValue,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$CaptureIds
    )

    $analysisPath = Join-FullPath $RunRoot "analysis.json"
    $Analysis | Add-Member -NotePropertyName "artifact_backlink" -NotePropertyValue ([ordered]@{
        schema_version = 1
        root_run_id = $RunIdValue
        root_manifest = "manifest.json"
        capture_ids = @($CaptureIds)
    }) -Force
    $Analysis | ConvertTo-Json -Depth 20 | Set-Content -Path $analysisPath -Encoding UTF8
    return ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $analysisPath
}

function Get-UiAuditBlockingIssues {
    param([AllowNull()]$Analysis)

    if ($null -eq $Analysis -or $null -eq $Analysis.PSObject.Properties["issues"]) {
        return @()
    }

    return @($Analysis.issues | Where-Object { [bool]$_.blocking -or $_.severity -in @("severe", "medium") })
}

function New-UiAuditFixPolicy {
    return [ordered]@{
        allowed_roots = @(
            "project/src/game/screens/",
            "project/src/framework/ui/",
            "project/src/game/navigation/",
            "project/assets/ui/themes/"
        )
        forbidden_roots = @(
            ".git/",
            "summary/",
            "target/",
            "project/target/",
            "android/app/build/",
            "android/build/"
            "tools/ui-visual-audit/fixtures/references/"
        )
        forbidden_file_names = @(
            ".env",
            ".env.local",
            ".env.production"
        )
        forbidden_name_patterns = @(
            "(?i)(^|[\\/])(secret|secrets|token|tokens|credential|credentials)([\\/\.]|$)",
            "(?i)\.(pem|p12|pfx|key)$"
        )
        forbidden_command_patterns = @(
            "(?i)(^|[\s;|&])(plan-baseline-update|apply-baseline-update|verify-baseline-rerun)([\s;|&]|$)",
            "(?i)(^|[\s;|&])(update-baseline|baseline-update)([\s;|&]|$)"
        )
        strategy_priority = @($script:FixStrategyPriority)
    }
}

function Test-UiAuditFixCommandBoundary {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)]$Policy
    )

    foreach ($pattern in @($Policy.forbidden_command_patterns)) {
        if ($Command -match [string]$pattern) {
            return [pscustomobject]@{
                allowed = $false
                failure_type = "baseline_update_forbidden"
                detail = "automatic fix commands cannot plan, apply, or verify baseline updates"
                matched_pattern = [string]$pattern
            }
        }
    }
    return [pscustomobject]@{
        allowed = $true
        failure_type = $null
        detail = "command does not invoke a baseline update entry point"
        matched_pattern = $null
    }
}

function ConvertTo-RepoRelativePath {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [AllowNull()][string]$PathValue
    )

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return [pscustomobject]@{ relative = $null; full = $null; outside_repo = $false; ignored = $true }
    }

    $raw = $PathValue.Trim()
    if ($raw -match "^[a-z][a-z0-9+.-]*://") {
        return [pscustomobject]@{ relative = $null; full = $null; outside_repo = $false; ignored = $true }
    }

    $repoFull = Get-FullPath $RepoRoot
    $full = if ([System.IO.Path]::IsPathRooted($raw)) {
        Get-FullPath $raw
    } else {
        Get-FullPath (Join-Path $repoFull $raw)
    }

    $repoPrefix = if ($repoFull.EndsWith([System.IO.Path]::DirectorySeparatorChar)) {
        $repoFull
    } else {
        $repoFull + [System.IO.Path]::DirectorySeparatorChar
    }

    if (-not $full.StartsWith($repoPrefix, [System.StringComparison]::OrdinalIgnoreCase) -and
        -not $full.Equals($repoFull, [System.StringComparison]::OrdinalIgnoreCase)) {
        return [pscustomobject]@{ relative = ($raw -replace "\\", "/"); full = $full; outside_repo = $true; ignored = $false }
    }

    $relative = Get-RelativePathCompat -BasePath $repoFull -TargetPath $full
    return [pscustomobject]@{ relative = ($relative -replace "\\", "/"); full = $full; outside_repo = $false; ignored = $false }
}

function Test-UiAuditPathUnderRoot {
    param(
        [Parameter(Mandatory = $true)][string]$RelativePath,
        [Parameter(Mandatory = $true)][string]$Root
    )

    $path = ($RelativePath -replace "\\", "/").TrimStart("/")
    $rootValue = ($Root -replace "\\", "/").TrimStart("/")
    if (-not $rootValue.EndsWith("/")) {
        $rootValue = "$rootValue/"
    }

    return $path.Equals($rootValue.TrimEnd("/"), [System.StringComparison]::OrdinalIgnoreCase) -or
        $path.StartsWith($rootValue, [System.StringComparison]::OrdinalIgnoreCase)
}

function Test-UiAuditFixPathAllowed {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$PathValue,
        [Parameter(Mandatory = $true)]$Policy
    )

    $resolved = ConvertTo-RepoRelativePath -RepoRoot $RepoRoot -PathValue $PathValue
    if ($resolved.ignored) {
        return [pscustomobject]@{ allowed = $true; path = $PathValue; relative = $null; reason = "ignored_non_file_reference" }
    }

    if ($resolved.outside_repo) {
        return [pscustomobject]@{ allowed = $false; path = $PathValue; relative = $resolved.relative; reason = "outside_repo" }
    }

    $relative = [string]$resolved.relative
    foreach ($root in @($Policy.forbidden_roots)) {
        if (Test-UiAuditPathUnderRoot -RelativePath $relative -Root ([string]$root)) {
            return [pscustomobject]@{ allowed = $false; path = $PathValue; relative = $relative; reason = "forbidden_root:$root" }
        }
    }

    $fileName = [System.IO.Path]::GetFileName($relative)
    foreach ($name in @($Policy.forbidden_file_names)) {
        if ($fileName.Equals([string]$name, [System.StringComparison]::OrdinalIgnoreCase)) {
            return [pscustomobject]@{ allowed = $false; path = $PathValue; relative = $relative; reason = "forbidden_file_name:$name" }
        }
    }

    foreach ($pattern in @($Policy.forbidden_name_patterns)) {
        if ($relative -match [string]$pattern) {
            return [pscustomobject]@{ allowed = $false; path = $PathValue; relative = $relative; reason = "forbidden_name_pattern" }
        }
    }

    foreach ($root in @($Policy.allowed_roots)) {
        if (Test-UiAuditPathUnderRoot -RelativePath $relative -Root ([string]$root)) {
            return [pscustomobject]@{ allowed = $true; path = $PathValue; relative = $relative; reason = "allowed_root:$root" }
        }
    }

    return [pscustomobject]@{ allowed = $false; path = $PathValue; relative = $relative; reason = "not_in_allowed_roots" }
}

function Test-UiAuditFixSafety {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [AllowEmptyCollection()][object[]]$Issues,
        [AllowEmptyCollection()][string[]]$ChangedPaths,
        [Parameter(Mandatory = $true)]$Policy
    )

    $paths = New-Object System.Collections.Generic.List[string]
    foreach ($issue in @($Issues)) {
        foreach ($file in @($issue.suggested_files)) {
            if (-not [string]::IsNullOrWhiteSpace([string]$file)) {
                $paths.Add([string]$file)
            }
        }
    }
    foreach ($path in @($ChangedPaths)) {
        if (-not [string]::IsNullOrWhiteSpace([string]$path)) {
            $paths.Add([string]$path)
        }
    }

    $checked = New-Object System.Collections.Generic.List[object]
    $violations = New-Object System.Collections.Generic.List[object]
    foreach ($path in @($paths.ToArray() | Select-Object -Unique)) {
        $result = Test-UiAuditFixPathAllowed -RepoRoot $RepoRoot -PathValue $path -Policy $Policy
        $checked.Add($result)
        if (-not [bool]$result.allowed) {
            $violations.Add($result)
        }
    }

    return [pscustomobject]@{
        allowed = ($violations.Count -eq 0)
        checked_paths = @($checked.ToArray())
        violations = @($violations.ToArray())
        policy = $Policy
    }
}

function New-UiAuditWatchedPathSet {
    param([Parameter(Mandatory = $true)]$Policy)

    $roots = New-Object System.Collections.Generic.List[string]
    foreach ($root in @($Policy.allowed_roots) + @($Policy.forbidden_roots)) {
        if (-not [string]::IsNullOrWhiteSpace([string]$root)) {
            $normalized = ([string]$root -replace "\\", "/").TrimStart("/")
            if (-not $normalized.EndsWith("/")) {
                $normalized = "$normalized/"
            }
            if (-not $roots.Contains($normalized)) {
                $roots.Add($normalized)
            }
        }
    }

    return @($roots.ToArray())
}

function Test-UiAuditPathWatchedByPolicy {
    param(
        [Parameter(Mandatory = $true)][string]$RelativePath,
        [Parameter(Mandatory = $true)]$Policy,
        [Parameter(Mandatory = $true)][string[]]$WatchedRoots
    )

    foreach ($root in @($WatchedRoots)) {
        if (Test-UiAuditPathUnderRoot -RelativePath $RelativePath -Root $root) {
            return $true
        }
    }

    $fileName = [System.IO.Path]::GetFileName($RelativePath)
    foreach ($name in @($Policy.forbidden_file_names)) {
        if ($fileName.Equals([string]$name, [System.StringComparison]::OrdinalIgnoreCase)) {
            return $true
        }
    }

    foreach ($pattern in @($Policy.forbidden_name_patterns)) {
        if ($RelativePath -match [string]$pattern) {
            return $true
        }
    }

    return $false
}

function Get-UiAuditPolicyFileSnapshot {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)]$Policy
    )

    $repoFull = Get-FullPath $RepoRoot
    $watchedRoots = @(New-UiAuditWatchedPathSet -Policy $Policy)
    $snapshot = @{}

    $scanRoots = New-Object System.Collections.Generic.List[string]
    foreach ($root in @($watchedRoots)) {
        $full = Join-FullPath $repoFull $root
        if ((Test-Path -LiteralPath $full) -and -not $scanRoots.Contains($full)) {
            $scanRoots.Add($full)
        }
    }

    foreach ($name in @($Policy.forbidden_file_names)) {
        $full = Join-FullPath $repoFull ([string]$name)
        if (Test-Path -LiteralPath $full) {
            $relative = Get-RelativePathCompat -BasePath $repoFull -TargetPath $full
            $item = Get-Item -LiteralPath $full -Force
            if (-not $item.PSIsContainer) {
                $snapshot[$relative] = [pscustomobject]@{
                    path = $relative
                    length = [int64]$item.Length
                    last_write_utc = $item.LastWriteTimeUtc.Ticks
                }
            }
        }
    }

    foreach ($root in @($scanRoots.ToArray())) {
        foreach ($item in @(Get-ChildItem -LiteralPath $root -File -Force -Recurse -ErrorAction SilentlyContinue)) {
            $relative = Get-RelativePathCompat -BasePath $repoFull -TargetPath $item.FullName
            if (Test-UiAuditPathWatchedByPolicy -RelativePath $relative -Policy $Policy -WatchedRoots $watchedRoots) {
                $snapshot[$relative] = [pscustomobject]@{
                    path = $relative
                    length = [int64]$item.Length
                    last_write_utc = $item.LastWriteTimeUtc.Ticks
                }
            }
        }
    }

    return $snapshot
}

function Compare-UiAuditPolicyFileSnapshot {
    param(
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$After
    )

    $changed = New-Object System.Collections.Generic.List[string]
    $allPaths = New-Object 'System.Collections.Generic.HashSet[string]'
    foreach ($path in $Before.Keys) {
        [void]$allPaths.Add([string]$path)
    }
    foreach ($path in $After.Keys) {
        [void]$allPaths.Add([string]$path)
    }

    foreach ($path in $allPaths) {
        if (-not $Before.ContainsKey($path) -or -not $After.ContainsKey($path)) {
            $changed.Add([string]$path)
            continue
        }

        $beforeEntry = $Before[$path]
        $afterEntry = $After[$path]
        if ([int64]$beforeEntry.length -ne [int64]$afterEntry.length -or
            [int64]$beforeEntry.last_write_utc -ne [int64]$afterEntry.last_write_utc) {
            $changed.Add([string]$path)
        }
    }

    return @($changed.ToArray())
}

function Merge-UiAuditChangedPaths {
    param([AllowEmptyCollection()][object[]]$PathSets)

    $merged = New-Object 'System.Collections.Generic.HashSet[string]'
    foreach ($set in @($PathSets)) {
        foreach ($path in @($set)) {
            if (-not [string]::IsNullOrWhiteSpace([string]$path)) {
                [void]$merged.Add(([string]$path -replace "\\", "/"))
            }
        }
    }

    return @($merged | Sort-Object)
}

function Get-UiAuditIssueScreens {
    param([AllowEmptyCollection()][object[]]$Issues)

    return @($Issues | ForEach-Object { [string]$_.screen } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Unique)
}

function Get-UiAuditIssueDevices {
    param([AllowEmptyCollection()][object[]]$Issues)

    return @($Issues | ForEach-Object { [string]$_.device } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Unique)
}

function New-UiAuditFixRerunPlan {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [AllowEmptyCollection()][object[]]$Issues
    )

    $screens = @(Get-UiAuditIssueScreens -Issues $Issues)
    if ($screens.Count -eq 0) {
        $screens = @($Manifest.screens)
    }

    if ([string]$Manifest.runner_mode -eq "remote") {
        $issueDevices = @(Get-UiAuditIssueDevices -Issues $Issues)
        $targets = @($Manifest.remote_targets | Where-Object {
            $label = [string]$_.label
            $deviceId = [string]$_.device_id
            $clientId = [string]$_.client_id
            $sessionId = [string]$_.session_id
            $issueDevices.Count -eq 0 -or $label -in $issueDevices -or $deviceId -in $issueDevices -or $clientId -in $issueDevices -or $sessionId -in $issueDevices
        })
        if ($targets.Count -eq 0) {
            $targets = @($Manifest.remote_targets)
        }

        return [ordered]@{
            mode = "remote_related_target_matrix"
            screens = @($screens)
            states = "auto"
            remote_targets = @($targets)
            expected_task_count = ($screens.Count * $targets.Count)
            command_shape = ".\scripts\run-ui-audit.ps1 -Mode Remote -Screens <screens> -DeviceId/-ClientId/-SessionId <related-targets> -States auto -FixMode Off"
        }
    }

    return [ordered]@{
        mode = "local_failed_screen_full_device_matrix"
        screens = @($screens)
        states = "auto"
        devices = @($script:BasicDevices)
        expected_task_count = ($screens.Count * $script:BasicDevices.Count)
        command_shape = ".\scripts\run-ui-audit.ps1 -Mode Local -Screens <screens> -Devices all -States auto -FixMode Off"
    }
}

function Copy-UiAuditIterationSnapshot {
    param(
        [Parameter(Mandatory = $true)][string]$SourceRoot,
        [Parameter(Mandatory = $true)][string]$SnapshotDir,
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)]$Manifest
    )

    New-Item -ItemType Directory -Force -Path $SnapshotDir | Out-Null
    foreach ($name in @("manifest.json", "report.md", "analysis.json", "analysis-input.json")) {
        $source = Join-FullPath $SourceRoot $name
        $destination = Join-FullPath $SnapshotDir $name
        if ((Test-Path $source) -and -not $source.Equals($destination, [System.StringComparison]::OrdinalIgnoreCase)) {
            Copy-Item -LiteralPath $source -Destination $destination -Force
        }
    }

    $artifactRoot = Join-FullPath $SnapshotDir "artifacts"
    $captureRefs = New-Object System.Collections.Generic.List[object]
    $index = 0
    foreach ($task in @($Manifest.tasks)) {
        foreach ($capture in @($task.captures)) {
            if ($null -eq $capture) {
                continue
            }

            $index += 1
            $safeName = "{0:D3}-{1}-{2}-{3}" -f $index, (Get-SafePathSegment ([string]$capture.screen)), (Get-SafePathSegment ([string]$capture.device)), (Get-SafePathSegment ([string]$capture.state))
            $screenshotCopy = $null
            $metadataCopy = $null

            foreach ($entry in @(
                    [pscustomobject]@{ kind = "screenshot"; value = if ($capture.PSObject.Properties["screenshot"]) { [string]$capture.screenshot } else { "" }; extension = ".png" },
                    [pscustomobject]@{ kind = "metadata"; value = if ($capture.PSObject.Properties["metadata"]) { [string]$capture.metadata } else { "" }; extension = ".json" }
                )) {
                if ([string]::IsNullOrWhiteSpace($entry.value) -or $entry.value -match "^[a-z][a-z0-9+.-]*://") {
                    continue
                }

                $sourcePath = if ([System.IO.Path]::IsPathRooted($entry.value)) {
                    Get-FullPath $entry.value
                } else {
                    Join-FullPath $SourceRoot $entry.value
                }
                if (-not (Test-Path $sourcePath)) {
                    continue
                }

                $targetDir = Join-FullPath $artifactRoot $entry.kind
                New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
                $target = Join-FullPath $targetDir "$safeName$($entry.extension)"
                if (-not $sourcePath.Equals($target, [System.StringComparison]::OrdinalIgnoreCase)) {
                    Copy-Item -LiteralPath $sourcePath -Destination $target -Force
                }
                if ($entry.kind -eq "screenshot") {
                    $screenshotCopy = ConvertTo-RunRelativePath -RunRoot $SnapshotDir -Path $target
                } else {
                    $metadataCopy = ConvertTo-RunRelativePath -RunRoot $SnapshotDir -Path $target
                }
            }

            $captureRefs.Add([pscustomobject]@{
                screen = [string]$capture.screen
                device = [string]$capture.device
                state = [string]$capture.state
                status = [string]$capture.status
                source_screenshot = if ($capture.PSObject.Properties["screenshot"]) { [string]$capture.screenshot } else { $null }
                source_metadata = if ($capture.PSObject.Properties["metadata"]) { [string]$capture.metadata } else { $null }
                screenshot_artifact_uri = if ($capture.PSObject.Properties["screenshot_artifact_uri"]) { [string]$capture.screenshot_artifact_uri } else { $null }
                metadata_artifact_uri = if ($capture.PSObject.Properties["metadata_artifact_uri"]) { [string]$capture.metadata_artifact_uri } else { $null }
                copied_screenshot = $screenshotCopy
                copied_metadata = $metadataCopy
            })
        }
    }

    $snapshot = [ordered]@{
        label = $Label
        created_at = (Get-Date).ToString("o")
        source_root = $SourceRoot
        capture_count = $captureRefs.Count
        captures = @($captureRefs.ToArray())
    }
    $snapshot | ConvertTo-Json -Depth 20 | Set-Content -Path (Join-FullPath $SnapshotDir "snapshot.json") -Encoding UTF8
    return [pscustomobject]@{
        label = $Label
        path = ConvertTo-RunRelativePath -RunRoot (Split-Path -Parent $SnapshotDir) -Path $SnapshotDir
        capture_count = $captureRefs.Count
        snapshot = "snapshot.json"
    }
}

function New-MockUiAuditFixLocalResult {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$Screen,
        [Parameter(Mandatory = $true)][string]$Device,
        [Parameter(Mandatory = $true)][string]$StateValue
    )

    $states = @(Split-UiAuditList @((Resolve-UiAuditStates -Screen $Screen -StateValue $StateValue)))
    $captures = New-Object System.Collections.Generic.List[object]
    $ordinal = 0
    foreach ($state in $states) {
        $ordinal += 1
        $safeState = Get-SafePathSegment $state
        $screenshot = Join-FullPath $RunRoot (Join-Path "screenshots" (Join-Path $Screen (Join-Path $Device ("{0:D2}-{1}.png" -f $ordinal, $safeState))))
        $metadata = Join-FullPath $RunRoot (Join-Path "metadata" (Join-Path $Screen (Join-Path $Device ("{0:D2}-{1}.json" -f $ordinal, $safeState))))
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $screenshot) | Out-Null
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $metadata) | Out-Null
        Set-Content -Path $screenshot -Value "mock-after-fix-png" -Encoding ASCII
        ([ordered]@{
            mock = $true
            screen = $Screen
            device = $Device
            state = $state
            fixed = $true
        }) | ConvertTo-Json -Depth 5 | Set-Content -Path $metadata -Encoding UTF8

        $captures.Add([pscustomobject]@{
            screen = $Screen
            requested_screen = $Screen
            device = $Device
            rendered_device = "mock-after-fix"
            state = $state
            status = "passed"
            failure = $null
            detail = $null
            screenshot = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $screenshot
            metadata = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $metadata
            screenshot_exists = $true
            metadata_exists = $true
            scroll_target_id = Get-RemoteScrollTargetId -Screen $Screen
            scroll_position = Get-RemoteScrollPosition -State $state
        })
    }

    return [pscustomobject]@{
        screen = $Screen
        requested_screen = $Screen
        device = $Device
        states = (Resolve-UiAuditStates -Screen $Screen -StateValue $StateValue)
        status = "passed"
        failure_type = $null
        detail = $null
        exit_code = 0
        timed_out = $false
        output_dir = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path (Join-FullPath $RunRoot (Join-Path "mock-runs" (Join-Path $Screen $Device)))
        stdout = $null
        stderr = $null
        child_manifest = $null
        child_report = $null
        cargo_args = @()
        bevy_args = @("--window-profile", $Device)
        captures = @($captures.ToArray())
    }
}

function New-MockUiAuditFixRemoteResult {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$RunIdValue,
        [Parameter(Mandatory = $true)][string]$Screen,
        [Parameter(Mandatory = $true)]$RemoteTarget,
        [Parameter(Mandatory = $true)][string]$StateValue
    )

    $task = New-RemoteUiAuditTask -RunRoot $RunRoot -Screen $Screen -RemoteTarget $RemoteTarget -StateValue $StateValue -TimeoutMs $RemoteCommandTimeoutMs
    $states = @(Split-UiAuditList @((Resolve-UiAuditStates -Screen $Screen -StateValue $StateValue)))
    $captures = New-Object System.Collections.Generic.List[object]
    $remoteTasks = New-Object System.Collections.Generic.List[object]
    $taskIds = New-Object System.Collections.Generic.List[string]
    $ordinal = 0
    foreach ($state in $states) {
        $ordinal += 1
        $safeState = Get-SafePathSegment $state
        $taskId = "mock_fix_$($RunIdValue)_$($Screen)_$($RemoteTarget.key)_$safeState"
        $taskIds.Add($taskId)
        $screenshot = Join-FullPath $RunRoot (Join-Path "remote-artifacts" (Join-Path $Screen (Join-Path $RemoteTarget.key ("$safeState-screenshot.png"))))
        $metadata = Join-FullPath $RunRoot (Join-Path "remote-artifacts" (Join-Path $Screen (Join-Path $RemoteTarget.key ("$safeState-metadata.json"))))
        $log = Join-FullPath $RunRoot (Join-Path "remote-artifacts" (Join-Path $Screen (Join-Path $RemoteTarget.key ("$safeState-client.log"))))
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $screenshot) | Out-Null
        Set-Content -Path $screenshot -Value "mock-remote-after-fix-png" -Encoding ASCII
        ([ordered]@{ mock = $true; screen = $Screen; target = $RemoteTarget.label; state = $state; fixed = $true }) | ConvertTo-Json -Depth 5 | Set-Content -Path $metadata -Encoding UTF8
        Set-Content -Path $log -Value "mock remote after fix log" -Encoding UTF8

        $artifactBase = "artifact://debug/$taskId"
        $captures.Add((New-RemoteCapture `
                    -Task $task `
                    -State $state `
                    -Status "passed" `
                    -Failure $null `
                    -Detail $null `
                    -ScreenshotArtifact ([pscustomobject]@{ uri = "$artifactBase/screenshot.png"; path = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $screenshot; exists = $true }) `
                    -MetadataArtifact ([pscustomobject]@{ uri = "$artifactBase/metadata.json"; path = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $metadata; exists = $true }) `
                    -LogArtifact ([pscustomobject]@{ uri = "$artifactBase/client.log"; path = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $log; exists = $true }) `
                    -ScreenshotTaskId $taskId `
                    -MetadataTaskId $taskId `
                    -LogTaskId $taskId `
                    -StateTaskIds @($taskId)))
        $remoteTasks.Add([pscustomobject]@{
            task_id = $taskId
            request_id = "mock_fix_request_$ordinal"
            command_type = "ui.screenshot"
            state = $state
            status = "succeeded"
            failure_type = $null
        })
    }

    return [pscustomobject]@{
        screen = $Screen
        requested_screen = $Screen
        device = [string]$RemoteTarget.label
        states = (Resolve-UiAuditStates -Screen $Screen -StateValue $StateValue)
        status = "passed"
        failure_type = $null
        detail = $null
        output_dir = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path ([string]$task.output_dir)
        remote_target = $RemoteTarget
        planned_commands = @($task.planned_commands)
        remote_tasks = @($remoteTasks.ToArray())
        task_ids = @($taskIds.ToArray())
        request_ids = @()
        captures = @($captures.ToArray())
    }
}

function Write-MockUiAuditFixRerun {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$RunIdValue,
        [Parameter(Mandatory = $true)]$OriginalManifest,
        [Parameter(Mandatory = $true)]$RerunPlan,
        [Parameter(Mandatory = $true)][string]$Scenario,
        [Parameter(Mandatory = $true)][object[]]$BlockingIssues
    )

    $results = New-Object System.Collections.Generic.List[object]
    if ([string]$OriginalManifest.runner_mode -eq "remote") {
        foreach ($screen in @($RerunPlan.screens)) {
            foreach ($target in @($RerunPlan.remote_targets)) {
                $results.Add((New-MockUiAuditFixRemoteResult -RunRoot $RunRoot -RunIdValue $RunIdValue -Screen ([string]$screen) -RemoteTarget $target -StateValue ([string]$RerunPlan.states)))
            }
        }
        $devices = @($RerunPlan.remote_targets | ForEach-Object { [string]$_.label })
        $passFixture = Join-FullPath $RunRoot "mock-analysis-pass.json"
        Write-FakeAnalysisResult -Path $passFixture -Issues @()
        Write-UiAuditRunnerOutputs -RunRoot $RunRoot -RunIdValue $RunIdValue -Results @($results.ToArray()) -ScreensValue @($RerunPlan.screens) -DevicesValue $devices -IsDryRun $false -RerunSource "fix-loop" -RunnerMode "Remote" -RemoteTargetsValue @($RerunPlan.remote_targets) -RemoteBackendName "Mock" -LocalDevicesValue @($OriginalManifest.local_devices) -AnalysisModeName "Fixture" -AnalysisResultFile $passFixture
        return Read-JsonFile (Join-FullPath $RunRoot "manifest.json")
    }

    foreach ($screen in @($RerunPlan.screens)) {
        foreach ($device in @($RerunPlan.devices)) {
            $results.Add((New-MockUiAuditFixLocalResult -RunRoot $RunRoot -Screen ([string]$screen) -Device ([string]$device) -StateValue ([string]$RerunPlan.states)))
        }
    }

    $analysisFixture = Join-FullPath $RunRoot "mock-analysis.json"
    if ($Scenario -eq "MaxIterations") {
        $firstResult = @($results.ToArray())[0]
        $firstCapture = @($firstResult.captures)[0]
        Write-FakeAnalysisResult -Path $analysisFixture -Issues @(
            (New-FakeAnalysisIssue -Capture $firstCapture -Severity "severe" -ProblemType "text_overlap" -Problem "mock issue persists after fix")
        )
    } else {
        Write-FakeAnalysisResult -Path $analysisFixture -Issues @()
    }

    Write-UiAuditRunnerOutputs -RunRoot $RunRoot -RunIdValue $RunIdValue -Results @($results.ToArray()) -ScreensValue @($RerunPlan.screens) -DevicesValue @($RerunPlan.devices) -IsDryRun $false -RerunSource "fix-loop" -RunnerMode "Local" -LocalDevicesValue @($RerunPlan.devices) -AnalysisModeName "Fixture" -AnalysisResultFile $analysisFixture
    return Read-JsonFile (Join-FullPath $RunRoot "manifest.json")
}

function Invoke-UiAuditProcess {
    param(
        [Parameter(Mandatory = $true)][string]$FileName,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][string]$StdoutLog,
        [Parameter(Mandatory = $true)][string]$StderrLog,
        [int]$TimeoutSeconds = 600,
        [hashtable]$Environment = @{}
    )

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $StdoutLog) | Out-Null
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $StderrLog) | Out-Null

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FileName
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    Set-ProcessArguments -ProcessStartInfo $startInfo -Arguments $Arguments
    foreach ($key in $Environment.Keys) {
        $startInfo.Environment[$key] = [string]$Environment[$key]
    }

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    try {
        if (-not $process.Start()) {
            return [pscustomobject]@{ started = $false; exit_code = $null; timed_out = $false; launch_error = "process did not start"; stdout = $StdoutLog; stderr = $StderrLog }
        }

        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            Stop-ProcessTreeCompat -Process $process
            Set-Content -Path $StdoutLog -Value $stdoutTask.GetAwaiter().GetResult() -Encoding UTF8
            Set-Content -Path $StderrLog -Value $stderrTask.GetAwaiter().GetResult() -Encoding UTF8
            return [pscustomobject]@{ started = $true; exit_code = $null; timed_out = $true; launch_error = $null; stdout = $StdoutLog; stderr = $StderrLog }
        }

        Set-Content -Path $StdoutLog -Value $stdoutTask.GetAwaiter().GetResult() -Encoding UTF8
        Set-Content -Path $StderrLog -Value $stderrTask.GetAwaiter().GetResult() -Encoding UTF8
        return [pscustomobject]@{ started = $true; exit_code = $process.ExitCode; timed_out = $false; launch_error = $null; stdout = $StdoutLog; stderr = $StderrLog }
    } catch {
        Set-Content -Path $StdoutLog -Value "" -Encoding UTF8
        Set-Content -Path $StderrLog -Value $_.Exception.Message -Encoding UTF8
        return [pscustomobject]@{ started = $false; exit_code = $null; timed_out = $false; launch_error = $_.Exception.Message; stdout = $StdoutLog; stderr = $StderrLog }
    } finally {
        $process.Dispose()
    }
}

function Invoke-UiAuditFixChecks {
    param(
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [Parameter(Mandatory = $true)][string]$IterationDir,
        [Parameter(Mandatory = $true)][string]$Mode,
        [Parameter(Mandatory = $true)][string]$Scenario
    )

    $checksDir = Join-FullPath $IterationDir "checks"
    New-Item -ItemType Directory -Force -Path $checksDir | Out-Null
    $fmtStdout = Join-FullPath $checksDir "cargo-fmt.stdout.log"
    $fmtStderr = Join-FullPath $checksDir "cargo-fmt.stderr.log"
    $checkStdout = Join-FullPath $checksDir "cargo-check.stdout.log"
    $checkStderr = Join-FullPath $checksDir "cargo-check.stderr.log"

    if ($Mode -eq "Mock") {
        Set-Content -Path $fmtStdout -Value "mock cargo fmt completed" -Encoding UTF8
        Set-Content -Path $fmtStderr -Value "" -Encoding UTF8
        if ($Scenario -eq "CheckFailed") {
            Set-Content -Path $checkStdout -Value "" -Encoding UTF8
            Set-Content -Path $checkStderr -Value "mock cargo check failed" -Encoding UTF8
            return [pscustomobject]@{
                status = "failed"
                failure_type = "fix_check_failed"
                commands = @(
                    [ordered]@{ command = "cargo fmt"; status = "passed"; exit_code = 0; stdout = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $fmtStdout; stderr = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $fmtStderr },
                    [ordered]@{ command = "cargo check"; status = "failed"; exit_code = 1; stdout = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $checkStdout; stderr = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $checkStderr }
                )
            }
        }

        Set-Content -Path $checkStdout -Value "mock cargo check completed" -Encoding UTF8
        Set-Content -Path $checkStderr -Value "" -Encoding UTF8
        return [pscustomobject]@{
            status = "passed"
            failure_type = $null
            commands = @(
                [ordered]@{ command = "cargo fmt"; status = "passed"; exit_code = 0; stdout = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $fmtStdout; stderr = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $fmtStderr },
                [ordered]@{ command = "cargo check"; status = "passed"; exit_code = 0; stdout = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $checkStdout; stderr = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $checkStderr }
            )
        }
    }

    $fmt = Invoke-UiAuditProcess -FileName "cargo" -Arguments @("fmt") -WorkingDirectory $ProjectRoot -StdoutLog $fmtStdout -StderrLog $fmtStderr -TimeoutSeconds 600
    if (-not [bool]$fmt.started -or [bool]$fmt.timed_out -or [int]$fmt.exit_code -ne 0) {
        return [pscustomobject]@{
            status = "failed"
            failure_type = "fix_check_failed"
            commands = @([ordered]@{ command = "cargo fmt"; status = "failed"; exit_code = $fmt.exit_code; timed_out = $fmt.timed_out; stdout = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $fmtStdout; stderr = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $fmtStderr })
        }
    }

    $check = Invoke-UiAuditProcess -FileName "cargo" -Arguments @("check") -WorkingDirectory $ProjectRoot -StdoutLog $checkStdout -StderrLog $checkStderr -TimeoutSeconds 1200
    $checkStatus = if ([bool]$check.started -and -not [bool]$check.timed_out -and [int]$check.exit_code -eq 0) { "passed" } else { "failed" }
    return [pscustomobject]@{
        status = $checkStatus
        failure_type = if ($checkStatus -eq "passed") { $null } else { "fix_check_failed" }
        commands = @(
            [ordered]@{ command = "cargo fmt"; status = "passed"; exit_code = $fmt.exit_code; timed_out = $fmt.timed_out; stdout = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $fmtStdout; stderr = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $fmtStderr },
            [ordered]@{ command = "cargo check"; status = $checkStatus; exit_code = $check.exit_code; timed_out = $check.timed_out; stdout = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $checkStdout; stderr = ConvertTo-RunRelativePath -RunRoot $IterationDir -Path $checkStderr }
        )
    }
}

function Get-GitStatusPathSet {
    param([Parameter(Mandatory = $true)][string]$RepoRoot)

    $paths = New-Object 'System.Collections.Generic.HashSet[string]'
    try {
        $lines = & git -C $RepoRoot status --porcelain --untracked-files=all 2>$null
        foreach ($line in @($lines)) {
            if ([string]::IsNullOrWhiteSpace($line) -or $line.Length -lt 4) {
                continue
            }
            $path = $line.Substring(3).Trim()
            if ($path -match " -> ") {
                $path = ($path -split " -> ")[-1]
            }
            [void]$paths.Add(($path -replace "\\", "/"))
        }
    } catch {
        return , $paths
    }
    return , $paths
}

function Compare-GitStatusPathSet {
    param(
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$After
    )

    $changed = New-Object System.Collections.Generic.List[string]
    foreach ($path in $After) {
        if (-not $Before.Contains($path)) {
            $changed.Add([string]$path)
        }
    }
    return @($changed.ToArray())
}

function Invoke-UiAuditFixCommand {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$IterationDir,
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string]$PlanPath
    )

    $stdout = Join-FullPath $IterationDir "fix-command.stdout.log"
    $stderr = Join-FullPath $IterationDir "fix-command.stderr.log"
    $env = @{
        MYBEVY_UI_AUDIT_FIX_PLAN = $PlanPath
        MYBEVY_UI_AUDIT_FIX_ITERATION_DIR = $IterationDir
        MYBEVY_UI_AUDIT_FIX_RUN_ROOT = (Split-Path -Parent (Split-Path -Parent $IterationDir))
    }
    return Invoke-UiAuditProcess -FileName "powershell" -Arguments @("-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", $Command) -WorkingDirectory $RepoRoot -StdoutLog $stdout -StderrLog $stderr -TimeoutSeconds 900 -Environment $env
}

function New-UiAuditFixLoopRecord {
    param(
        [Parameter(Mandatory = $true)][string]$Mode,
        [Parameter(Mandatory = $true)][int]$MaxIterations,
        [Parameter(Mandatory = $true)]$Policy,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Issues
    )

    return [ordered]@{
        schema_version = 1
        mode = $Mode
        status = "running"
        pass = $false
        failure_type = $null
        detail = $null
        max_fix_iterations = $MaxIterations
        started_at = (Get-Date).ToString("o")
        completed_at = $null
        initial_blocking_issue_count = $Issues.Count
        strategy_priority = @($Policy.strategy_priority)
        safety_policy = $Policy
        before = $null
        iterations = @()
        final_issues = @($Issues)
    }
}

function Complete-UiAuditFixLoopRecord {
    param(
        [Parameter(Mandatory = $true)]$Record,
        [Parameter(Mandatory = $true)][string]$Status,
        [AllowNull()][string]$FailureType,
        [AllowNull()][string]$Detail,
        [AllowEmptyCollection()][object[]]$FinalIssues
    )

    $Record.status = $Status
    $Record.pass = ($Status -eq "passed")
    $Record.failure_type = $FailureType
    $Record.detail = $Detail
    $Record.completed_at = (Get-Date).ToString("o")
    $Record.final_issues = @($FinalIssues)
    return $Record
}

function Update-UiAuditManifestWithFixLoop {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$RunIdValue,
        [Parameter(Mandatory = $true)]$FixLoop
    )

    $manifestPath = Join-FullPath $RunRoot "manifest.json"
    $manifest = Read-JsonFile $manifestPath
    $manifest | Add-Member -NotePropertyName "fix_loop" -NotePropertyValue $FixLoop -Force
    if ($FixLoop.status -eq "passed") {
        $manifest.status = "passed"
    }
    $fixLinks = New-Object System.Collections.Generic.List[object]
    foreach ($iteration in @($FixLoop.iterations)) {
        if ([string]::IsNullOrWhiteSpace([string]$iteration.after_manifest)) {
            continue
        }
        $manifestFull = Join-FullPath $RunRoot ([string]$iteration.after_manifest)
        $analysisFull = if ([string]::IsNullOrWhiteSpace([string]$iteration.after_analysis)) { $null } else { Join-FullPath $RunRoot ([string]$iteration.after_analysis) }
        $reportFull = if ([string]::IsNullOrWhiteSpace([string]$iteration.after_report)) { $null } else { Join-FullPath $RunRoot ([string]$iteration.after_report) }
        $fixLinks.Add([ordered]@{
            iteration = [int]$iteration.iteration
            manifest = New-UiAuditArtifactLink -RunRoot $RunRoot -Path $manifestFull
            analysis = if ($analysisFull -and (Test-Path -LiteralPath $analysisFull -PathType Leaf)) { New-UiAuditArtifactLink -RunRoot $RunRoot -Path $analysisFull } else { $null }
            report = if ($reportFull -and (Test-Path -LiteralPath $reportFull -PathType Leaf)) { New-UiAuditArtifactLink -RunRoot $RunRoot -Path $reportFull } else { $null }
        })
    }
    if ($null -eq $manifest.PSObject.Properties["artifact_links"]) {
        $manifest | Add-Member -NotePropertyName "artifact_links" -NotePropertyValue ([pscustomobject]@{}) -Force
    }
    $manifest.artifact_links | Add-Member -NotePropertyName "fix_iterations" -NotePropertyValue @($fixLinks.ToArray()) -Force
    $manifest | ConvertTo-Json -Depth 30 | Set-Content -Path $manifestPath -Encoding UTF8
    Build-UiAuditReport -RunRoot $RunRoot -RunIdValue $RunIdValue -Manifest $manifest | Set-Content -Path (Join-FullPath $RunRoot "report.md") -Encoding UTF8
    return $manifest
}

function Invoke-UiAuditFixLoop {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$RunIdValue,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [Parameter(Mandatory = $true)][string]$Mode,
        [Parameter(Mandatory = $true)][int]$MaxIterations,
        [AllowNull()][string]$Command,
        [Parameter(Mandatory = $true)][string]$MockScenario
    )

    if ($Mode -eq "Off") {
        return [pscustomobject]@{ status = "skipped"; pass = $true; failure_type = $null; exit_code = 0; detail = "fix loop disabled" }
    }
    if ($MaxIterations -lt 1) {
        throw "MaxFixIterations must be at least 1."
    }

    $manifest = Read-JsonFile (Join-FullPath $RunRoot "manifest.json")
    $analysis = if ($null -ne $manifest.PSObject.Properties["analysis"]) { $manifest.analysis } else { $null }
    $blockingIssues = @(Get-UiAuditBlockingIssues -Analysis $analysis)
    if ($blockingIssues.Count -eq 0) {
        $policy = New-UiAuditFixPolicy
        $record = New-UiAuditFixLoopRecord -Mode $Mode -MaxIterations $MaxIterations -Policy $policy -Issues @()
        $record = Complete-UiAuditFixLoopRecord -Record $record -Status "skipped" -FailureType $null -Detail "no blocking analysis issue; fix loop was not started" -FinalIssues @()
        Update-UiAuditManifestWithFixLoop -RunRoot $RunRoot -RunIdValue $RunIdValue -FixLoop $record | Out-Null
        return [pscustomobject]@{ status = "skipped"; pass = $true; failure_type = $null; exit_code = 0; detail = $record.detail }
    }

    $policy = New-UiAuditFixPolicy
    $scenarioChangedPaths = @()
    if ($Mode -eq "Mock" -and $MockScenario -eq "UnsafePath") {
        $scenarioChangedPaths = @("summary/ui-audit/mock-forbidden.rs")
    }
    $safety = Test-UiAuditFixSafety -RepoRoot $RepoRoot -Issues $blockingIssues -ChangedPaths $scenarioChangedPaths -Policy $policy
    $record = New-UiAuditFixLoopRecord -Mode $Mode -MaxIterations $MaxIterations -Policy $policy -Issues $blockingIssues
    $iterationsRoot = Join-FullPath $RunRoot "iterations"
    New-Item -ItemType Directory -Force -Path $iterationsRoot | Out-Null
    $beforeDir = Join-FullPath $iterationsRoot "00-before"
    $beforeSnapshot = Copy-UiAuditIterationSnapshot -SourceRoot $RunRoot -SnapshotDir $beforeDir -Label "before" -Manifest $manifest
    $record.before = [ordered]@{
        path = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $beforeDir
        snapshot = "snapshot.json"
        capture_count = $beforeSnapshot.capture_count
    }

    if (-not [bool]$safety.allowed) {
        $record.safety_result = $safety
        $record = Complete-UiAuditFixLoopRecord -Record $record -Status "failed" -FailureType "safety_policy_rejected" -Detail "blocking analysis suggested files outside the UI fix allowlist" -FinalIssues $blockingIssues
        Update-UiAuditManifestWithFixLoop -RunRoot $RunRoot -RunIdValue $RunIdValue -FixLoop $record | Out-Null
        return [pscustomobject]@{ status = "failed"; pass = $false; failure_type = "safety_policy_rejected"; exit_code = 1; detail = $record.detail }
    }

    $rerunPlan = New-UiAuditFixRerunPlan -Manifest $manifest -Issues $blockingIssues
    if ($Mode -eq "Plan") {
        $planPath = Join-FullPath $iterationsRoot "fix-plan.json"
        ([ordered]@{
            mode = "Plan"
            issues = @($blockingIssues)
            strategy_priority = @($policy.strategy_priority)
            safety = $safety
            rerun_plan = $rerunPlan
            checks = @("cargo fmt", "cargo check")
        }) | ConvertTo-Json -Depth 30 | Set-Content -Path $planPath -Encoding UTF8
        $record.plan = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $planPath
        $record.rerun_plan = $rerunPlan
        $record = Complete-UiAuditFixLoopRecord -Record $record -Status "planned" -FailureType $null -Detail "fix loop plan generated; no code was modified" -FinalIssues $blockingIssues
        Update-UiAuditManifestWithFixLoop -RunRoot $RunRoot -RunIdValue $RunIdValue -FixLoop $record | Out-Null
        return [pscustomobject]@{ status = "planned"; pass = $false; failure_type = $null; exit_code = 1; detail = $record.detail }
    }

    $iterationRecords = New-Object System.Collections.Generic.List[object]
    $lastIssues = @($blockingIssues)
    for ($iteration = 1; $iteration -le $MaxIterations; $iteration += 1) {
        $iterationDir = Join-FullPath $iterationsRoot ("{0:D2}-after-fix" -f $iteration)
        New-Item -ItemType Directory -Force -Path $iterationDir | Out-Null
        $planPath = Join-FullPath $iterationDir "fix-plan.json"
        ([ordered]@{
            iteration = $iteration
            mode = $Mode
            issues = @($lastIssues)
            strategy_priority = @($policy.strategy_priority)
            safety = $safety
            rerun_plan = $rerunPlan
            fix_command = if ([string]::IsNullOrWhiteSpace($Command)) { $null } else { $Command }
        }) | ConvertTo-Json -Depth 30 | Set-Content -Path $planPath -Encoding UTF8

        $iterationRecord = [ordered]@{
            iteration = $iteration
            path = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $iterationDir
            status = "running"
            failure_type = $null
            fix_plan = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $planPath
            selected_strategy = "page_local_layout"
            fixer = $null
            safety = $safety
            checks = $null
            rerun_plan = $rerunPlan
            after_manifest = $null
            after_report = $null
            after_analysis = $null
            after_snapshot = $null
        }

        if ($Mode -eq "Command") {
            if ([string]::IsNullOrWhiteSpace($Command)) {
                $iterationRecord.status = "failed"
                $iterationRecord.failure_type = "fix_command_missing"
                $iterationRecord.detail = "FixMode Command requires -FixCommand."
                $iterationRecords.Add([pscustomobject]$iterationRecord)
                $record.iterations = @($iterationRecords.ToArray())
                $record = Complete-UiAuditFixLoopRecord -Record $record -Status "failed" -FailureType "fix_command_missing" -Detail $iterationRecord.detail -FinalIssues $lastIssues
                Update-UiAuditManifestWithFixLoop -RunRoot $RunRoot -RunIdValue $RunIdValue -FixLoop $record | Out-Null
                return [pscustomobject]@{ status = "failed"; pass = $false; failure_type = "fix_command_missing"; exit_code = 1; detail = $iterationRecord.detail }
            }
            $commandBoundary = Test-UiAuditFixCommandBoundary -Command $Command -Policy $policy
            if (-not [bool]$commandBoundary.allowed) {
                $iterationRecord.status = "failed"
                $iterationRecord.failure_type = [string]$commandBoundary.failure_type
                $iterationRecord.detail = [string]$commandBoundary.detail
                $iterationRecord.fixer = [ordered]@{
                    mode = "Command"
                    command = $Command
                    status = "rejected"
                    boundary = $commandBoundary
                }
                $iterationRecords.Add([pscustomobject]$iterationRecord)
                $record.iterations = @($iterationRecords.ToArray())
                $record = Complete-UiAuditFixLoopRecord -Record $record -Status "failed" -FailureType $iterationRecord.failure_type -Detail $iterationRecord.detail -FinalIssues $lastIssues
                Update-UiAuditManifestWithFixLoop -RunRoot $RunRoot -RunIdValue $RunIdValue -FixLoop $record | Out-Null
                return [pscustomobject]@{ status = "failed"; pass = $false; failure_type = $iterationRecord.failure_type; exit_code = 1; detail = $iterationRecord.detail }
            }
            $beforePaths = Get-GitStatusPathSet -RepoRoot $RepoRoot
            $beforePolicySnapshot = Get-UiAuditPolicyFileSnapshot -RepoRoot $RepoRoot -Policy $policy
            $commandResult = Invoke-UiAuditFixCommand -RepoRoot $RepoRoot -IterationDir $iterationDir -Command $Command -PlanPath $planPath
            $afterPaths = Get-GitStatusPathSet -RepoRoot $RepoRoot
            $afterPolicySnapshot = Get-UiAuditPolicyFileSnapshot -RepoRoot $RepoRoot -Policy $policy
            $gitChangedPaths = @(Compare-GitStatusPathSet -Before $beforePaths -After $afterPaths)
            $policyChangedPaths = @(Compare-UiAuditPolicyFileSnapshot -Before $beforePolicySnapshot -After $afterPolicySnapshot)
            $newChangedPaths = @(Merge-UiAuditChangedPaths -PathSets @($gitChangedPaths, $policyChangedPaths))
            $postSafety = Test-UiAuditFixSafety -RepoRoot $RepoRoot -Issues @() -ChangedPaths $newChangedPaths -Policy $policy
            $iterationRecord.fixer = [ordered]@{
                mode = "Command"
                command = $Command
                status = if ([bool]$commandResult.started -and -not [bool]$commandResult.timed_out -and [int]$commandResult.exit_code -eq 0) { "passed" } else { "failed" }
                exit_code = $commandResult.exit_code
                timed_out = $commandResult.timed_out
                stdout = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path ([string]$commandResult.stdout)
                stderr = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path ([string]$commandResult.stderr)
                git_changed_paths = @($gitChangedPaths)
                policy_changed_paths = @($policyChangedPaths)
                new_changed_paths = @($newChangedPaths)
            }
            $iterationRecord.safety = $postSafety
            if ($iterationRecord.fixer.status -ne "passed") {
                $iterationRecord.status = "failed"
                $iterationRecord.failure_type = "fix_command_failed"
                $iterationRecords.Add([pscustomobject]$iterationRecord)
                $record.iterations = @($iterationRecords.ToArray())
                $record = Complete-UiAuditFixLoopRecord -Record $record -Status "failed" -FailureType "fix_command_failed" -Detail "fix command failed before cargo checks" -FinalIssues $lastIssues
                Update-UiAuditManifestWithFixLoop -RunRoot $RunRoot -RunIdValue $RunIdValue -FixLoop $record | Out-Null
                return [pscustomobject]@{ status = "failed"; pass = $false; failure_type = "fix_command_failed"; exit_code = 1; detail = $record.detail }
            }
            if (-not [bool]$postSafety.allowed) {
                $iterationRecord.status = "failed"
                $iterationRecord.failure_type = "safety_policy_rejected"
                $iterationRecords.Add([pscustomobject]$iterationRecord)
                $record.iterations = @($iterationRecords.ToArray())
                $record = Complete-UiAuditFixLoopRecord -Record $record -Status "failed" -FailureType "safety_policy_rejected" -Detail "fix command changed files outside the UI fix allowlist" -FinalIssues $lastIssues
                Update-UiAuditManifestWithFixLoop -RunRoot $RunRoot -RunIdValue $RunIdValue -FixLoop $record | Out-Null
                return [pscustomobject]@{ status = "failed"; pass = $false; failure_type = "safety_policy_rejected"; exit_code = 1; detail = $record.detail }
            }
        } else {
            $changedFile = @($lastIssues | ForEach-Object { $_.suggested_files } | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) } | Select-Object -First 1)
            if ($changedFile.Count -eq 0) {
                $changedFile = @("project/src/game/screens/dev/ui_gallery.rs")
            }
            $fixerOutput = [ordered]@{
                mode = "Mock"
                status = "passed"
                simulated = $true
                changed_files = @($changedFile)
                selected_strategy = "page_local_layout"
                note = "mock fixer records the intended UI code change without touching source files"
            }
            $fixerOutput | ConvertTo-Json -Depth 10 | Set-Content -Path (Join-FullPath $iterationDir "fixer-output.json") -Encoding UTF8
            $iterationRecord.fixer = $fixerOutput
        }

        $checks = Invoke-UiAuditFixChecks -ProjectRoot $ProjectRoot -IterationDir $iterationDir -Mode $Mode -Scenario $MockScenario
        $iterationRecord.checks = $checks
        if ($checks.status -ne "passed") {
            $iterationRecord.status = "failed"
            $iterationRecord.failure_type = "fix_check_failed"
            $iterationRecords.Add([pscustomobject]$iterationRecord)
            $record.iterations = @($iterationRecords.ToArray())
            $record = Complete-UiAuditFixLoopRecord -Record $record -Status "failed" -FailureType "fix_check_failed" -Detail "cargo fmt or cargo check failed after fix" -FinalIssues $lastIssues
            Update-UiAuditManifestWithFixLoop -RunRoot $RunRoot -RunIdValue $RunIdValue -FixLoop $record | Out-Null
            return [pscustomobject]@{ status = "failed"; pass = $false; failure_type = "fix_check_failed"; exit_code = 1; detail = $record.detail }
        }

        if ($Mode -eq "Command") {
            $iterationRecord.status = "planned"
            $iterationRecord.detail = "command fix and cargo checks completed; rerun plan is recorded for the next integration step"
            $iterationRecords.Add([pscustomobject]$iterationRecord)
            $record.iterations = @($iterationRecords.ToArray())
            $record = Complete-UiAuditFixLoopRecord -Record $record -Status "planned" -FailureType $null -Detail $iterationRecord.detail -FinalIssues $lastIssues
            Update-UiAuditManifestWithFixLoop -RunRoot $RunRoot -RunIdValue $RunIdValue -FixLoop $record | Out-Null
            return [pscustomobject]@{ status = "planned"; pass = $false; failure_type = $null; exit_code = 1; detail = $record.detail }
        }

        $afterRunId = "$RunIdValue-fix-$iteration"
        $afterManifest = Write-MockUiAuditFixRerun -RunRoot $iterationDir -RunIdValue $afterRunId -OriginalManifest $manifest -RerunPlan $rerunPlan -Scenario $MockScenario -BlockingIssues $lastIssues
        $afterManifestPath = Join-FullPath $iterationDir "manifest.json"
        $afterManifest | Add-Member -NotePropertyName "artifact_backlink" -NotePropertyValue ([ordered]@{
            schema_version = 1
            root_run_id = $RunIdValue
            root_manifest = ConvertTo-RunRelativePath -RunRoot $iterationDir -Path (Join-FullPath $RunRoot "manifest.json")
            capture_ids = @($afterManifest.tasks | ForEach-Object { $_.captures } | Where-Object { $null -ne $_ } | ForEach-Object { "$($_.screen).$($_.device).$($_.state)" } | Select-Object -Unique)
        }) -Force
        $afterManifest | ConvertTo-Json -Depth 30 | Set-Content -Path $afterManifestPath -Encoding UTF8
        $iterationRecord.after_manifest = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path (Join-FullPath $iterationDir "manifest.json")
        $iterationRecord.after_report = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path (Join-FullPath $iterationDir "report.md")
        $iterationRecord.after_analysis = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path (Join-FullPath $iterationDir "analysis.json")
        $afterSnapshot = Copy-UiAuditIterationSnapshot -SourceRoot $iterationDir -SnapshotDir $iterationDir -Label "after" -Manifest $afterManifest
        $iterationRecord.after_snapshot = [ordered]@{
            path = ConvertTo-RunRelativePath -RunRoot $RunRoot -Path $iterationDir
            snapshot = "snapshot.json"
            capture_count = $afterSnapshot.capture_count
        }

        $afterIssues = @(Get-UiAuditBlockingIssues -Analysis $afterManifest.analysis)
        $lastIssues = @($afterIssues)
        if ($afterIssues.Count -eq 0) {
            $iterationRecord.status = "passed"
            $iterationRecords.Add([pscustomobject]$iterationRecord)
            $record.iterations = @($iterationRecords.ToArray())
            $record = Complete-UiAuditFixLoopRecord -Record $record -Status "passed" -FailureType $null -Detail "blocking UI analysis issues cleared after fix iteration $iteration" -FinalIssues @()
            Update-UiAuditManifestWithFixLoop -RunRoot $RunRoot -RunIdValue $RunIdValue -FixLoop $record | Out-Null
            return [pscustomobject]@{ status = "passed"; pass = $true; failure_type = $null; exit_code = 0; detail = $record.detail }
        }

        $iterationRecord.status = "failed"
        $iterationRecord.failure_type = "ai_blocking_issue"
        $iterationRecord.remaining_issues = @($afterIssues)
        $iterationRecords.Add([pscustomobject]$iterationRecord)
    }

    $record.iterations = @($iterationRecords.ToArray())
    $record = Complete-UiAuditFixLoopRecord -Record $record -Status "failed" -FailureType "max_iterations_reached" -Detail "maximum fix iterations reached with blocking issues still present" -FinalIssues $lastIssues
    Update-UiAuditManifestWithFixLoop -RunRoot $RunRoot -RunIdValue $RunIdValue -FixLoop $record | Out-Null
    return [pscustomobject]@{ status = "failed"; pass = $false; failure_type = "max_iterations_reached"; exit_code = 1; detail = $record.detail }
}

function Resolve-UiAuditRunnerExitCode {
    param(
        [Parameter(Mandatory = $true)][object[]]$Results,
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$RunIdValue,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$ProjectRoot
    )

    $failed = @($Results | Where-Object { $_.status -eq "failed" })
    if ($failed.Count -gt 0) {
        return 1
    }

    $runnerManifestPath = Join-FullPath $RunRoot "manifest.json"
    if (Test-Path -LiteralPath $runnerManifestPath -PathType Leaf) {
        $runnerManifest = Read-JsonFile $runnerManifestPath
        if ($null -ne $runnerManifest.PSObject.Properties["comparison"] -and
            $null -ne $runnerManifest.comparison -and
            [string]$runnerManifest.comparison.status -eq "failed") {
            return 1
        }
        if ($null -ne $runnerManifest.PSObject.Properties["android_validation"] -and
            [bool]$runnerManifest.android_validation.required -and
            [string]$runnerManifest.android_validation.status -ne "passed") {
            return 1
        }
    }

    if ($script:LastUiAuditAnalysisStatus -and $script:LastUiAuditAnalysisStatus.status -eq "failed") {
        if ($script:LastUiAuditAnalysisStatus.failure_type -eq "ai_blocking_issue" -and $FixMode -ne "Off") {
            Write-Host "Blocking UI analysis issue found. Starting fix loop ($FixMode, max iterations: $MaxFixIterations)."
            $fixResult = Invoke-UiAuditFixLoop `
                -RunRoot $RunRoot `
                -RunIdValue $RunIdValue `
                -RepoRoot $RepoRoot `
                -ProjectRoot $ProjectRoot `
                -Mode $FixMode `
                -MaxIterations $MaxFixIterations `
                -Command $FixCommand `
                -MockScenario $MockFixScenario
            if ($fixResult.status -eq "passed") {
                Write-Host "Fix loop passed."
            } else {
                Write-Host "Fix loop failed: $($fixResult.failure_type) $($fixResult.detail)"
            }
            return [int]$fixResult.exit_code
        }

        if ($FixMode -ne "Off") {
            $fixResult = Invoke-UiAuditFixLoop `
                -RunRoot $RunRoot `
                -RunIdValue $RunIdValue `
                -RepoRoot $RepoRoot `
                -ProjectRoot $ProjectRoot `
                -Mode $FixMode `
                -MaxIterations $MaxFixIterations `
                -Command $FixCommand `
                -MockScenario $MockFixScenario
            if ($fixResult.status -eq "skipped") {
                Write-Host "Fix loop skipped: $($fixResult.detail)"
            }
        }

        return 1
    }

    if ($FixMode -ne "Off") {
        $fixResult = Invoke-UiAuditFixLoop `
            -RunRoot $RunRoot `
            -RunIdValue $RunIdValue `
            -RepoRoot $RepoRoot `
            -ProjectRoot $ProjectRoot `
            -Mode $FixMode `
            -MaxIterations $MaxFixIterations `
            -Command $FixCommand `
            -MockScenario $MockFixScenario
        if ($fixResult.status -eq "skipped") {
            Write-Host "Fix loop skipped: $($fixResult.detail)"
        }
    }

    return 0
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

function ConvertTo-RepositoryRelativePath {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $relative = Get-RelativePathCompat -BasePath $RepositoryRoot -TargetPath $Path
    if ($relative.StartsWith("../", [System.StringComparison]::Ordinal) -or $relative -eq "..") {
        throw "Audit evidence must remain inside the repository: $Path"
    }
    return $relative
}

function New-UiAuditRepositoryArtifactLink {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $full = Get-FullPath $Path
    if (-not (Test-Path -LiteralPath $full -PathType Leaf)) {
        throw "Required audit artifact is missing: $full"
    }
    return [ordered]@{
        path = ConvertTo-RepositoryRelativePath -RepositoryRoot $RepositoryRoot -Path $full
        sha256 = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}

function Test-UiAuditGitLfsPointer {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $false
    }
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -eq 0) {
        return $false
    }
    $prefixLength = [Math]::Min(128, $bytes.Length)
    $prefix = [System.Text.Encoding]::ASCII.GetString($bytes, 0, $prefixLength)
    return $prefix.StartsWith("version https://git-lfs.github.com/spec/v1", [System.StringComparison]::Ordinal)
}

function Invoke-UiAuditVisualTool {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$LogName,
        [int[]]$AcceptedExitCodes = @(0)
    )

    $toolManifest = Join-FullPath $RepositoryRoot $script:UiAuditComparisonToolManifest
    if (-not (Test-Path -LiteralPath $toolManifest -PathType Leaf)) {
        throw "UI visual audit tool manifest is missing: $toolManifest"
    }
    $logDirectory = Join-FullPath $RunRoot "logs/comparison"
    New-Item -ItemType Directory -Force -Path $logDirectory | Out-Null
    $logPath = Join-FullPath $logDirectory "$LogName.log"
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $cacheBinary = Join-FullPath $RepositoryRoot "tools/ui-visual-audit/target/debug/ui-visual-audit.exe"
    $cacheStatus = if (Test-Path -LiteralPath $cacheBinary -PathType Leaf) { "UI visual audit cache hit: $cacheBinary" } else { "UI visual audit cache cold; cargo will build $Command" }
    Write-Host $cacheStatus
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = "cargo"
    $psi.WorkingDirectory = $RepositoryRoot
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    Set-ProcessArguments -ProcessStartInfo $psi -Arguments (@("run", "--quiet", "--manifest-path", $toolManifest, "--", $Command) + $Arguments)
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $psi
    try {
        [void]$process.Start()
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $completed = $process.WaitForExit([Math]::Max(1, $ComparisonBudgetSeconds) * 1000)
        if (-not $completed) {
            Stop-ProcessTreeCompat -Process $process
            [void]$process.WaitForExit(10000)
            $timeoutOutput = ($stdoutTask.GetAwaiter().GetResult() + [Environment]::NewLine + $stderrTask.GetAwaiter().GetResult())
            Set-Content -LiteralPath $logPath -Value $timeoutOutput -Encoding UTF8
            $stopwatch.Stop()
            throw "ui-visual-audit $Command timed out after $ComparisonBudgetSeconds seconds. See $logPath."
        }
        $process.WaitForExit()
        $output = $stdoutTask.GetAwaiter().GetResult() + [Environment]::NewLine + $stderrTask.GetAwaiter().GetResult()
        $exitCode = $process.ExitCode
    } finally {
        $process.Dispose()
    }
    $stopwatch.Stop()
    Set-Content -LiteralPath $logPath -Value $output -Encoding UTF8
    if ($exitCode -notin $AcceptedExitCodes) {
        $detail = (@($output -split "`r?`n") | Select-Object -Last 8) -join [Environment]::NewLine
        throw "ui-visual-audit $Command failed with exit code $exitCode. See $logPath. $detail"
    }
    return [pscustomobject]@{
        exit_code = $exitCode
        elapsed_milliseconds = [int64]$stopwatch.ElapsedMilliseconds
        log = $logPath
    }
}

function Get-UiAuditReferenceEntries {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$ManifestPath,
        [Parameter(Mandatory = $true)][string]$LocaleValue,
        [Parameter(Mandatory = $true)][string]$ThemeValue
    )

    $fullManifest = Get-FullPath $ManifestPath
    if (-not (Test-Path -LiteralPath $fullManifest -PathType Leaf)) {
        throw "Reference manifest not found: $fullManifest"
    }
    $manifest = Read-JsonFile $fullManifest
    if ($manifest.schema_version -ne $script:UiAuditReferenceManifestSchemaVersion -or $null -eq $manifest.references) {
        throw "Reference manifest must use schema_version $($script:UiAuditReferenceManifestSchemaVersion) and contain references: $fullManifest"
    }

    $locale = $LocaleValue.Trim().ToLowerInvariant().Replace("-", "_")
    $theme = $ThemeValue.Trim()
    $entries = New-Object System.Collections.Generic.List[object]
    foreach ($reference in @($manifest.references)) {
        if ([string]$reference.key.locale -ne $locale -or [string]$reference.key.theme -ne $theme) {
            continue
        }
        if ([string]$reference.key.device -notin $script:BasicDevices) {
            throw "Reference '$($reference.reference_id)' uses unsupported runner device '$($reference.key.device)'."
        }
        [void](Resolve-UiAuditScreens @([string]$reference.key.screen))
        $deviceProfile = Get-UiAuditDeterministicProfile -Device ([string]$reference.key.device)
        if ([int]$reference.viewport.physical_size.width -ne [int]$deviceProfile.physical_width -or
            [int]$reference.viewport.physical_size.height -ne [int]$deviceProfile.physical_height -or
            [Math]::Abs(([double]$reference.viewport.device_scale) - ([double]$deviceProfile.device_scale)) -gt 0.000001) {
            throw "Reference '$($reference.reference_id)' viewport does not match deterministic profile '$($reference.key.device)'."
        }
        $root = if ([string]$reference.image.storage -eq "committed_fixture") {
            Join-FullPath $RepositoryRoot "tools/ui-visual-audit/fixtures/references"
        } elseif ([string]$reference.image.storage -eq "temporary_local") {
            Join-FullPath $RepositoryRoot "summary/ui-visual-audit"
        } else {
            throw "Reference '$($reference.reference_id)' has unsupported storage '$($reference.image.storage)'."
        }
        $imagePath = Join-FullPath $root ([string]$reference.image.relative_path)
        if (-not (Test-Path -LiteralPath $imagePath -PathType Leaf)) {
            throw "Reference '$($reference.reference_id)' is missing: $imagePath. Fetch approved baselines with 'git lfs pull' or restore the temporary local reference."
        }
        if (Test-UiAuditGitLfsPointer -Path $imagePath) {
            throw "Reference '$($reference.reference_id)' is a Git LFS pointer, not an image: $imagePath. Run 'git lfs pull' before strict comparison."
        }
        $entries.Add([pscustomobject]@{
                reference = $reference
                image_path = $imagePath
            })
    }
    if ($entries.Count -eq 0) {
        throw "Reference manifest contains no entries for locale '$locale' and theme '$theme'."
    }
    return @($entries.ToArray())
}

function Get-UiAuditReferenceTaskSeeds {
    param([Parameter(Mandatory = $true)][object[]]$References)

    $seeds = New-Object System.Collections.Generic.List[object]
    foreach ($group in @($References | Group-Object { "$($_.reference.key.screen)|$($_.reference.key.device)" })) {
        $first = @($group.Group)[0].reference
        $states = @($group.Group | ForEach-Object { [string]$_.reference.key.state } | Select-Object -Unique)
        $seeds.Add([pscustomobject]@{
                screen = [string]$first.key.screen
                device = [string]$first.key.device
                states = ($states -join ",")
            })
    }
    return @($seeds.ToArray())
}

function New-UiAuditThresholdProfile {
    param(
        [Parameter(Mandatory = $true)][double]$MaximumChangedRatio,
        [ValidateRange(1, 65535)][int]$Weight = 100
    )

    $maximum = [Math]::Min(1000000, [Math]::Max(0, [int]([Math]::Round($MaximumChangedRatio * 1000000))))
    return [ordered]@{
        weight = $Weight
        max_raw_changed_ratio_millionths = $maximum
        max_alpha_changed_ratio_millionths = $maximum
        max_tolerated_changed_ratio_millionths = $maximum
        minimum_ssim_millionths = 0
        max_geometry_changed_ratio_millionths = $maximum
        max_large_area_ratio_millionths = $maximum
    }
}

function ConvertTo-UiAuditGateThreshold {
    param([Parameter(Mandatory = $true)]$RegionThreshold)
    return [ordered]@{
        max_raw_changed_ratio_millionths = [int]$RegionThreshold.max_raw_changed_ratio_millionths
        max_alpha_changed_ratio_millionths = [int]$RegionThreshold.max_alpha_changed_ratio_millionths
        max_tolerated_changed_ratio_millionths = [int]$RegionThreshold.max_tolerated_changed_ratio_millionths
        minimum_ssim_millionths = [int]$RegionThreshold.minimum_ssim_millionths
        max_geometry_changed_ratio_millionths = [int]$RegionThreshold.max_geometry_changed_ratio_millionths
        max_large_area_ratio_millionths = [int]$RegionThreshold.max_large_area_ratio_millionths
    }
}

function New-UiAuditBoundReport {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$Path
    )
    return New-UiAuditRepositoryArtifactLink -RepositoryRoot $RepositoryRoot -Path $Path
}

function Get-UiAuditCapturedReferencePairs {
    param(
        [Parameter(Mandatory = $true)][object[]]$Results,
        [Parameter(Mandatory = $true)][object[]]$References,
        [Parameter(Mandatory = $true)][string]$RunRoot
    )

    $capturesById = @{}
    foreach ($capture in @($Results | ForEach-Object { $_.captures } | Where-Object { $null -ne $_ })) {
        if ([string]$capture.status -ne "passed" -or -not [bool]$capture.screenshot_exists -or -not [bool]$capture.metadata_exists) {
            continue
        }
        if ($capture.repetition_index -gt 1) {
            continue
        }
        $captureId = "$($capture.screen).$($capture.device).$($capture.state)"
        if (-not $capturesById.ContainsKey($captureId)) {
            $capturesById[$captureId] = $capture
        }
    }

    $pairs = New-Object System.Collections.Generic.List[object]
    $missing = New-Object System.Collections.Generic.List[object]
    foreach ($entry in $References) {
        $reference = $entry.reference
        $captureId = "$($reference.key.screen).$($reference.key.device).$($reference.key.state)"
        if (-not $capturesById.ContainsKey($captureId)) {
            $missing.Add([pscustomobject]@{
                    screen = [string]$reference.key.screen
                    device = [string]$reference.key.device
                    state = [string]$reference.key.state
                    reason = "capture artifact is missing or failed"
                })
            continue
        }
        $capture = $capturesById[$captureId]
        $actual = Resolve-ArtifactPath -Value ([string]$capture.screenshot) -TaskOutputDir $RunRoot
        $metadata = Resolve-ArtifactPath -Value ([string]$capture.metadata) -TaskOutputDir $RunRoot
        if (-not (Test-Path -LiteralPath $actual -PathType Leaf) -or -not (Test-Path -LiteralPath $metadata -PathType Leaf)) {
            $missing.Add([pscustomobject]@{
                    screen = [string]$reference.key.screen
                    device = [string]$reference.key.device
                    state = [string]$reference.key.state
                    reason = "capture artifact path is missing"
                })
            continue
        }
        $pairs.Add([pscustomobject]@{
                capture_id = $captureId
                capture = $capture
                reference = $reference
                reference_path = [string]$entry.image_path
                actual_path = $actual
                metadata_path = $metadata
            })
    }
    return [pscustomobject]@{ pairs = @($pairs.ToArray()); missing = @($missing.ToArray()) }
}

function Write-UiAuditJsonFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    $Value | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $Path -Encoding UTF8
}

function Get-UiAuditComparisonArguments {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$RunRoot
    )
    return @(
        "--repository-root", $RepositoryRoot,
        "--allowed-input-root", $RepositoryRoot,
        "--allowed-input-root", $RunRoot,
        "--allowed-output-root", $RunRoot
    )
}

function Invoke-UiAuditReferencePairComparison {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)]$Pair,
        [Parameter(Mandatory = $true)][string]$ReferenceManifestPath,
        [Parameter(Mandatory = $true)][string]$ComparisonAiModeValue
    )

    $reference = $Pair.reference
    $captureId = [string]$Pair.capture_id
    $segment = Get-SafePathSegment $captureId
    $workingRoot = Join-FullPath $RunRoot (Join-Path "comparison" $segment)
    $configRoot = Join-FullPath $workingRoot "configs"
    $baseArgs = Get-UiAuditComparisonArguments -RepositoryRoot $RepositoryRoot -RunRoot $RunRoot
    $referenceBinding = [ordered]@{
        sha256 = [string]$reference.image.sha256
        revision = [int]$reference.baseline.version
    }
    $threshold = New-UiAuditThresholdProfile -MaximumChangedRatio ([double]$reference.allowed_differences.max_changed_pixel_ratio)
    $criticalThreshold = New-UiAuditThresholdProfile -MaximumChangedRatio ([double]$reference.allowed_differences.max_changed_pixel_ratio) -Weight 100
    $normalThreshold = New-UiAuditThresholdProfile -MaximumChangedRatio ([double]$reference.allowed_differences.max_changed_pixel_ratio) -Weight 40
    $decorativeThreshold = New-UiAuditThresholdProfile -MaximumChangedRatio ([double]$reference.allowed_differences.max_changed_pixel_ratio) -Weight 10
    $gateThreshold = ConvertTo-UiAuditGateThreshold -RegionThreshold $threshold
    $width = [int]$reference.metadata.original_size.width
    $height = [int]$reference.metadata.original_size.height
    $compareConfig = Join-FullPath $configRoot "exact.config.json"
    $normalizeConfig = Join-FullPath $configRoot "normalize.config.json"
    $diffConfig = Join-FullPath $configRoot "diff.config.json"
    $regionConfig = Join-FullPath $configRoot "regions.config.json"
    $semanticConfig = Join-FullPath $configRoot "semantic.config.json"
    $gateConfig = Join-FullPath $configRoot "gate.config.json"
    Write-UiAuditJsonFile -Path $compareConfig -Value ([ordered]@{
            schema_version = 1
            algorithm_version = "exact_rgba_v1"
            max_changed_pixel_ratio = [double]$reference.allowed_differences.max_changed_pixel_ratio
        })
    Write-UiAuditJsonFile -Path $normalizeConfig -Value ([ordered]@{
            schema_version = 1
            algorithm_version = "normalize_align_v1"
            orientation_policy = "apply_exif"
            color_policy = "srgb_only"
            alpha_policy = "straight_zero_transparent_rgb"
            reference = [ordered]@{ crop = [ordered]@{ kind = "none" } }
            actual = [ordered]@{ crop = [ordered]@{ kind = "none" } }
            alignment = [ordered]@{ mode = "integer_search"; maximum_translation = [ordered]@{ x = 2; y = 2 } }
        })
    Write-UiAuditJsonFile -Path $diffConfig -Value ([ordered]@{
            schema_version = 1
            algorithm_version = "ui_diff_metrics_v1"
            over_threshold_channel_abs = 8
            small_channel_tolerance = 3
            edge_antialias_tolerance = 12
            edge_luma_threshold = 96
            ssim_window_size = 8
            large_area_min_pixels = 16
            large_area_min_ratio_millionths = 1000
        })
    Write-UiAuditJsonFile -Path $regionConfig -Value ([ordered]@{
            schema_version = 1
            algorithm_version = "ui_region_audit_v1"
            reference_binding = $referenceBinding
            audit_scope = "full_image"
            maximum_ignored_ratio_millionths = 0
            threshold_profiles = [ordered]@{ critical = $criticalThreshold; normal = $normalThreshold; decorative = $decorativeThreshold }
            bounds_sources = @()
            regions = @([ordered]@{
                    region_id = "full_capture"
                    label = "Full capture"
                    semantic_role = "content"
                    level = "normal"
                    clipping = "reject_out_of_bounds"
                    source = [ordered]@{
                        kind = "manual"
                        coordinate_space = "aligned"
                        shape = [ordered]@{ kind = "rectangle"; bounds = [ordered]@{ x = 0; y = 0; width = $width; height = $height } }
                    }
                })
            ignore_regions = @()
        })
    Write-UiAuditJsonFile -Path $semanticConfig -Value ([ordered]@{
            schema_version = 1
            algorithm_version = "ui_semantic_audit_v1"
            minimum_touch_width = 44.0
            minimum_touch_height = 44.0
            geometry_epsilon = 0.015625
            text_overlap_minimum_area = 1.0
            require_safe_area_for_roles = @("critical_text", "button", "icon_button", "text_input")
        })
    Write-UiAuditJsonFile -Path $gateConfig -Value ([ordered]@{
            schema_version = 1
            algorithm_version = "ui_visual_gate_v1"
            conservative_default = [ordered]@{ critical = $gateThreshold; normal = $gateThreshold; decorative = $gateThreshold }
            reference_profiles = @([ordered]@{
                    profile_id = [string]$reference.allowed_differences.profile
                    reference_binding = $referenceBinding
                    thresholds = [ordered]@{ critical = $gateThreshold; normal = $gateThreshold; decorative = $gateThreshold }
                    calibration_fixture_id = "repository-reference-manifest"
                    adjustment_rationale = "Threshold is bound to the approved reference manifest allowed_differences policy."
                })
        })

    $compareOutput = Join-FullPath $workingRoot "compare"
    $normalizeOutput = Join-FullPath $workingRoot "normalize"
    $diffOutput = Join-FullPath $workingRoot "diff"
    $regionOutput = Join-FullPath $workingRoot "regions"
    $semanticOutput = Join-FullPath $workingRoot "semantic"
    $toolRuns = New-Object System.Collections.Generic.List[object]
    $toolRuns.Add((Invoke-UiAuditVisualTool -RepositoryRoot $RepositoryRoot -RunRoot $RunRoot -Command "compare" -LogName "$segment-compare" -AcceptedExitCodes @(0, 4) -Arguments ($baseArgs + @("--reference", $Pair.reference_path, "--actual", $Pair.actual_path, "--config", $compareConfig, "--output-directory", $compareOutput))))
    $toolRuns.Add((Invoke-UiAuditVisualTool -RepositoryRoot $RepositoryRoot -RunRoot $RunRoot -Command "normalize-align" -LogName "$segment-normalize" -Arguments ($baseArgs + @("--reference", $Pair.reference_path, "--actual", $Pair.actual_path, "--normalization-manifest", $normalizeConfig, "--output-directory", $normalizeOutput))))
    $alignedReference = Join-FullPath $normalizeOutput "aligned-reference.png"
    $alignedActual = Join-FullPath $normalizeOutput "aligned-actual.png"
    $normalizationReport = Join-FullPath $normalizeOutput "normalization-report.json"
    $toolRuns.Add((Invoke-UiAuditVisualTool -RepositoryRoot $RepositoryRoot -RunRoot $RunRoot -Command "analyze-diff" -LogName "$segment-diff" -Arguments ($baseArgs + @("--reference", $alignedReference, "--actual", $alignedActual, "--config", $diffConfig, "--output-directory", $diffOutput))))
    $diffReport = Join-FullPath $diffOutput "diff-metrics-report.json"
    $toolRuns.Add((Invoke-UiAuditVisualTool -RepositoryRoot $RepositoryRoot -RunRoot $RunRoot -Command "audit-regions" -LogName "$segment-regions" -AcceptedExitCodes @(0, 4) -Arguments ($baseArgs + @("--reference", $alignedReference, "--actual", $alignedActual, "--diff-config", $diffConfig, "--region-config", $regionConfig, "--normalization-report", $normalizationReport, "--output-directory", $regionOutput))))
    $regionReport = Join-FullPath $regionOutput "region-audit-report.json"
    $toolRuns.Add((Invoke-UiAuditVisualTool -RepositoryRoot $RepositoryRoot -RunRoot $RunRoot -Command "audit-semantics" -LogName "$segment-semantic" -AcceptedExitCodes @(0, 4) -Arguments ($baseArgs + @("--metadata", $Pair.metadata_path, "--config", $semanticConfig, "--output-directory", $semanticOutput))))
    $semanticReport = Join-FullPath $semanticOutput "semantic-audit-report.json"

    $aiReport = $null
    if ($ComparisonAiModeValue -ne "Off") {
        $aiResponse = Join-FullPath $configRoot "fixture-response.json"
        $aiConfig = Join-FullPath $configRoot "ai.config.json"
        $aiBundle = Join-FullPath $configRoot "ai.bundle.json"
        $aiOutput = Join-FullPath $workingRoot "ai"
        if ($ComparisonAiModeValue -eq "Provider") {
            $providerConfig = [string]$env:MYBEVY_UI_AUDIT_AI_CONFIG
            if ([string]::IsNullOrWhiteSpace($providerConfig) -or -not (Test-Path -LiteralPath $providerConfig -PathType Leaf)) {
                throw "ComparisonAiMode Provider requires MYBEVY_UI_AUDIT_AI_CONFIG to name an explicit provider config. CI never enables it implicitly."
            }
            Copy-Item -LiteralPath $providerConfig -Destination $aiConfig -Force
        } else {
            Write-UiAuditJsonFile -Path $aiResponse -Value ([ordered]@{ schema_version = 1; issues = @() })
            Write-UiAuditJsonFile -Path $aiConfig -Value ([ordered]@{
                    schema_version = 1
                    algorithm_version = "ui_ai_visual_analysis_v1"
                    provider = [ordered]@{
                        mode = "fixture"
                        provider_id = "runner-fixture-ai"
                        audit_model_id = "runner-fixture-audit-v1"
                        generation_model_id = "runner-fixture-generation-v1"
                        response = $aiResponse
                    }
                    policy = [ordered]@{
                        attempt_timeout_ms = 1000
                        minimum_request_interval_ms = 0
                        max_attempts = 1
                        initial_backoff_ms = 0
                        max_backoff_ms = 0
                        max_output_tokens = 1024
                    }
                })
        }
        Write-UiAuditJsonFile -Path $aiBundle -Value ([ordered]@{
                schema_version = 1
                run_id = "strict-$captureId"
                captures = @([ordered]@{
                        capture_id = $captureId
                        screen = [string]$reference.key.screen
                        device = [string]$reference.key.device
                        state = [string]$reference.key.state
                        images = [ordered]@{
                            reference = $alignedReference
                            actual = $alignedActual
                            overlay = (Join-FullPath $diffOutput "overlay.png")
                            heatmap = (Join-FullPath $diffOutput "heatmap.png")
                        }
                        diff_metrics = $diffReport
                        region_metrics = $regionReport
                        semantic_report = $semanticReport
                        ui_metadata = $Pair.metadata_path
                        allowed_differences = [ordered]@{ profile = [string]$reference.allowed_differences.profile; notes = @($reference.allowed_differences.notes) }
                        likely_files = @()
                        privacy = [ordered]@{ redact_semantic_text = $true; redaction_rects = @() }
                    })
            })
        $toolRuns.Add((Invoke-UiAuditVisualTool -RepositoryRoot $RepositoryRoot -RunRoot $RunRoot -Command "analyze-ai" -LogName "$segment-ai" -Arguments ($baseArgs + @("--bundle", $aiBundle, "--config", $aiConfig, "--output-directory", $aiOutput))))
        $aiReport = Join-FullPath $aiOutput "ai-analysis-report.json"
    }

    $gateBundle = Join-FullPath $configRoot "gate.bundle.json"
    Write-UiAuditJsonFile -Path $gateBundle -Value ([ordered]@{
            schema_version = 1
            run_id = "strict-$captureId"
            captures = @([ordered]@{
                    capture_id = $captureId
                    screen = [string]$reference.key.screen
                    device = [string]$reference.key.device
                    state = [string]$reference.key.state
                    reference_profile = [string]$reference.allowed_differences.profile
                    reference_binding = $referenceBinding
                    diff_report = (New-UiAuditBoundReport -RepositoryRoot $RepositoryRoot -Path $diffReport)
                    region_report = (New-UiAuditBoundReport -RepositoryRoot $RepositoryRoot -Path $regionReport)
                    semantic_report = (New-UiAuditBoundReport -RepositoryRoot $RepositoryRoot -Path $semanticReport)
                })
            ai_report = if ($null -eq $aiReport) { $null } else { (New-UiAuditBoundReport -RepositoryRoot $RepositoryRoot -Path $aiReport) }
        })
    $gateOutput = Join-FullPath $workingRoot "gate"
    $toolRuns.Add((Invoke-UiAuditVisualTool -RepositoryRoot $RepositoryRoot -RunRoot $RunRoot -Command "evaluate-gate" -LogName "$segment-gate" -AcceptedExitCodes @(0, 2, 3, 4) -Arguments ($baseArgs + @("--bundle", $gateBundle, "--config", $gateConfig, "--output-directory", $gateOutput))))

    foreach ($path in @((Join-FullPath $compareOutput "comparison-report.json"), $normalizationReport, $diffReport, $regionReport, $semanticReport, (Join-FullPath $gateOutput "visual-gate-report.json"), (Join-FullPath $diffOutput "overlay.png"), (Join-FullPath $diffOutput "heatmap.png"))) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Comparison tool completed without required artifact: $path"
        }
    }
    return [pscustomobject]@{
        pair = $Pair
        capture_id = $captureId
        reference_binding = $referenceBinding
        compare_report = (Join-FullPath $compareOutput "comparison-report.json")
        normalization_report = $normalizationReport
        diff_report = $diffReport
        region_report = $regionReport
        semantic_report = $semanticReport
        ai_report = $aiReport
        gate_report = (Join-FullPath $gateOutput "visual-gate-report.json")
        reference_path = $Pair.reference_path
        actual_path = $Pair.actual_path
        overlay_path = (Join-FullPath $diffOutput "overlay.png")
        heatmap_path = (Join-FullPath $diffOutput "heatmap.png")
        tool_runs = @($toolRuns.ToArray())
    }
}

function New-UiAuditComparisonMetricSummary {
    param([Parameter(Mandatory = $true)]$Metrics)
    return [ordered]@{
        raw_changed_ratio_millionths = [int]$Metrics.raw.changed_pixel_ratio_millionths
        alpha_changed_ratio_millionths = [int]$Metrics.alpha.changed_pixel_ratio_millionths
        tolerated_changed_ratio_millionths = [int]$Metrics.tolerated.changed_pixel_ratio_millionths
        ssim_millionths = [int]$Metrics.perceptual.score_millionths
        geometry_changed_ratio_millionths = [int]$Metrics.categories.geometry_edges.mismatched_edge_ratio_millionths
        large_area_ratio_millionths = [int]$Metrics.categories.large_area_content.covered_pixel_ratio_millionths
    }
}

function New-UiAuditComparisonResultCapture {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$ReferenceManifestPath,
        [Parameter(Mandatory = $true)]$Comparison
    )

    $diff = Read-JsonFile $Comparison.diff_report
    $regions = Read-JsonFile $Comparison.region_report
    $gate = Read-JsonFile $Comparison.gate_report
    $ai = if ($null -eq $Comparison.ai_report) { $null } else { Read-JsonFile $Comparison.ai_report }
    $reference = $Comparison.pair.reference
    $regionSummaries = New-Object System.Collections.Generic.List[object]
    foreach ($region in @($regions.region_results)) {
        $regionSummaries.Add([ordered]@{
                region_id = [string]$region.region_id
                level = [string]$region.level
                bounds = $region.mapped_aligned_bounds
                status = [string]$region.local_status
                metrics = (New-UiAuditComparisonMetricSummary -Metrics $region.metrics)
                threshold = [ordered]@{
                    profile = [string]$reference.allowed_differences.profile
                    values = [ordered]@{
                        max_raw_changed_ratio_millionths = [int]$region.threshold.max_raw_changed_ratio_millionths
                        max_alpha_changed_ratio_millionths = [int]$region.threshold.max_alpha_changed_ratio_millionths
                        max_tolerated_changed_ratio_millionths = [int]$region.threshold.max_tolerated_changed_ratio_millionths
                        minimum_ssim_millionths = [int]$region.threshold.minimum_ssim_millionths
                        max_geometry_changed_ratio_millionths = [int]$region.threshold.max_geometry_changed_ratio_millionths
                        max_large_area_ratio_millionths = [int]$region.threshold.max_large_area_ratio_millionths
                    }
                }
            })
    }
    $thresholdValues = [ordered]@{
        max_raw_changed_ratio_millionths = [int]$regions.region_results[0].threshold.max_raw_changed_ratio_millionths
        max_alpha_changed_ratio_millionths = [int]$regions.region_results[0].threshold.max_alpha_changed_ratio_millionths
        max_tolerated_changed_ratio_millionths = [int]$regions.region_results[0].threshold.max_tolerated_changed_ratio_millionths
        minimum_ssim_millionths = [int]$regions.region_results[0].threshold.minimum_ssim_millionths
        max_geometry_changed_ratio_millionths = [int]$regions.region_results[0].threshold.max_geometry_changed_ratio_millionths
        max_large_area_ratio_millionths = [int]$regions.region_results[0].threshold.max_large_area_ratio_millionths
    }
    $gateState = ([string]$gate.status).ToLowerInvariant()
    $issues = New-Object System.Collections.Generic.List[object]
    foreach ($reason in @($gate.captures[0].reasons)) {
        if ([string]$reason.failure_type -eq "none") {
            continue
        }
        $issues.Add([ordered]@{
                issue_id = "gate-$($issues.Count + 1)"
                source = "visual_gate"
                region_id = $null
                severity = if ([bool]$reason.blocking) { "severe" } else { "minor" }
                message = [string]$reason.message
                evidence = [ordered]@{ image_role = "overlay"; rect = $null; description = [string]$reason.source_id }
                node_id = $null
                source_path = $null
                likely_files = @()
                likely_cause = $null
                suggested_change_scope = $null
            })
    }
    return [ordered]@{
        capture_id = [string]$Comparison.capture_id
        screen = [string]$reference.key.screen
        device = [string]$reference.key.device
        state = [string]$reference.key.state
        reference_binding = $Comparison.reference_binding
        artifacts = [ordered]@{
            reference = (New-UiAuditRepositoryArtifactLink -RepositoryRoot $RepositoryRoot -Path $Comparison.reference_path)
            actual = (New-UiAuditRepositoryArtifactLink -RepositoryRoot $RepositoryRoot -Path $Comparison.actual_path)
            overlay = (New-UiAuditRepositoryArtifactLink -RepositoryRoot $RepositoryRoot -Path $Comparison.overlay_path)
            heatmap = (New-UiAuditRepositoryArtifactLink -RepositoryRoot $RepositoryRoot -Path $Comparison.heatmap_path)
        }
        metrics = (New-UiAuditComparisonMetricSummary -Metrics $diff.metrics)
        regions = @($regionSummaries.ToArray())
        masks = @()
        allowed_differences = [ordered]@{
            profile = [string]$reference.allowed_differences.profile
            notes = @($reference.allowed_differences.notes)
        }
        algorithms = [ordered]@{
            compare = "exact_rgba_v1"
            normalize = "normalize_align_v1"
            diff = "ui_diff_metrics_v1"
            regions = "ui_region_audit_v1"
            semantic = "ui_semantic_audit_v1"
            gate = "ui_visual_gate_v1"
        }
        thresholds = @([ordered]@{ profile = [string]$reference.allowed_differences.profile; values = $thresholdValues })
        ai = if ($null -eq $ai) {
            [ordered]@{ ran = $false; provider_id = $null; model_id = $null; issue_count = 0 }
        } else {
            [ordered]@{ ran = $true; provider_id = [string]$ai.provider.provider_id; model_id = [string]$ai.provider.audit_model_id; issue_count = @($ai.issues).Count }
        }
        gate_state = $gateState
        issues = @($issues.ToArray())
        baseline_guard = [ordered]@{
            reference_id = [string]$reference.reference_id
            reference_manifest = (New-UiAuditRepositoryArtifactLink -RepositoryRoot $RepositoryRoot -Path $ReferenceManifestPath)
            expected = $Comparison.reference_binding
            observed = $Comparison.reference_binding
            approval_receipt = $null
        }
    }
}

function Complete-UiAuditReferenceComparison {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$RunIdValue,
        [Parameter(Mandatory = $true)][object[]]$Results,
        [Parameter(Mandatory = $true)][bool]$IsDryRun
    )

    if ([string]::IsNullOrWhiteSpace($ReferenceManifest)) {
        return $null
    }
    if (-not $StrictReference) {
        throw "-ReferenceManifest requires -StrictReference so ordinary audit runs cannot silently become baseline comparisons."
    }
    if (-not $DeterministicCapture -and -not $IsDryRun) {
        throw "Strict reference comparison requires -DeterministicCapture."
    }
    if ($ComparisonBudgetSeconds -le 0 -or $ComparisonBudgetPeakMemoryBytes -le 0 -or $ComparisonBudgetArtifactBytes -le 0) {
        throw "Comparison budgets must be positive."
    }

    $referenceManifestPath = Get-FullPath $ReferenceManifest
    $validate = Invoke-UiAuditVisualTool -RepositoryRoot $RepositoryRoot -RunRoot $RunRoot -Command "validate-manifest" -LogName "reference-manifest" -Arguments @("--repository-root", $RepositoryRoot, "--manifest", $referenceManifestPath)
    $references = @(Get-UiAuditReferenceEntries -RepositoryRoot $RepositoryRoot -ManifestPath $referenceManifestPath -LocaleValue $Locale -ThemeValue $Theme)
    $manifestPath = Join-FullPath $RunRoot "manifest.json"
    $manifest = Read-JsonFile $manifestPath
    $referenceLink = New-UiAuditRepositoryArtifactLink -RepositoryRoot $RepositoryRoot -Path $referenceManifestPath
    if ($IsDryRun) {
        $manifest | Add-Member -NotePropertyName "comparison" -NotePropertyValue ([ordered]@{
                mode = "strict_reference"
                status = "planned"
                reference_manifest = $referenceLink
                matrix_total = $references.Count
                ai_mode = "fixture"
                detail = "dry run did not produce screenshots or comparison evidence"
            }) -Force
        $manifest | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
        return $manifest.comparison
    }

    $overall = [System.Diagnostics.Stopwatch]::StartNew()
    $pairSet = Get-UiAuditCapturedReferencePairs -Results $Results -References $references -RunRoot $RunRoot
    $completed = New-Object System.Collections.Generic.List[object]
    $failures = New-Object System.Collections.Generic.List[object]
    foreach ($missing in @($pairSet.missing)) {
        $failures.Add($missing)
    }
    foreach ($pair in @($pairSet.pairs)) {
        try {
            Write-Host "Comparing $($pair.capture_id) against strict reference $($pair.reference.reference_id)"
            $comparison = Invoke-UiAuditReferencePairComparison -RepositoryRoot $RepositoryRoot -RunRoot $RunRoot -Pair $pair -ReferenceManifestPath $referenceManifestPath -ComparisonAiModeValue $ComparisonAiMode
            $completed.Add($comparison)
        } catch {
            $failures.Add([pscustomobject]@{
                    screen = [string]$pair.reference.key.screen
                    device = [string]$pair.reference.key.device
                    state = [string]$pair.reference.key.state
                    reason = $_.Exception.Message
                })
        }
    }
    $overall.Stop()
    $artifactBytes = if (Test-Path -LiteralPath (Join-FullPath $RunRoot "comparison")) {
        [int64](@(Get-ChildItem -LiteralPath (Join-FullPath $RunRoot "comparison") -Recurse -File | Measure-Object -Property Length -Sum).Sum)
    } else { [int64]0 }
    $peakMemory = [int64]0
    foreach ($comparison in @($completed.ToArray())) {
        foreach ($reportPath in @($comparison.diff_report, $comparison.semantic_report, $comparison.gate_report)) {
            $report = Read-JsonFile $reportPath
            $candidate = if ($null -ne $report.performance -and $null -ne $report.performance.peak_working_memory) { [int64]$report.performance.peak_working_memory.bytes } elseif ($null -ne $report.performance -and $null -ne $report.performance.estimated_peak_memory_bytes) { [int64]$report.performance.estimated_peak_memory_bytes } else { 0 }
            $peakMemory = [Math]::Max($peakMemory, $candidate)
        }
    }
    if ($overall.Elapsed.TotalSeconds -gt $ComparisonBudgetSeconds) {
        $failures.Add([pscustomobject]@{ screen = "matrix"; device = "all"; state = "all"; reason = "comparison matrix exceeded $ComparisonBudgetSeconds seconds" })
    }
    if ($peakMemory -gt $ComparisonBudgetPeakMemoryBytes) {
        $failures.Add([pscustomobject]@{ screen = "matrix"; device = "all"; state = "all"; reason = "estimated peak memory $peakMemory exceeds budget $ComparisonBudgetPeakMemoryBytes" })
    }
    if ($artifactBytes -gt $ComparisonBudgetArtifactBytes) {
        $failures.Add([pscustomobject]@{ screen = "matrix"; device = "all"; state = "all"; reason = "comparison artifacts $artifactBytes exceed budget $ComparisonBudgetArtifactBytes" })
    }

    $captureResults = New-Object System.Collections.Generic.List[object]
    foreach ($comparison in @($completed.ToArray())) {
        try {
            $captureResults.Add((New-UiAuditComparisonResultCapture -RepositoryRoot $RepositoryRoot -ReferenceManifestPath $referenceManifestPath -Comparison $comparison))
        } catch {
            $failures.Add([pscustomobject]@{
                    screen = [string]$comparison.pair.reference.key.screen
                    device = [string]$comparison.pair.reference.key.device
                    state = [string]$comparison.pair.reference.key.state
                    reason = "comparison report input assembly failed: $($_.Exception.Message)"
                })
        }
    }
    $bundlePath = Join-FullPath $RunRoot "comparison/comparison-bundle.json"
    $comparisonInputLink = $null
    $resultLink = $null
    $reportLink = $null
    if ($failures.Count -eq 0 -and $captureResults.Count -gt 0) {
        $analysisPath = Join-FullPath $RunRoot "analysis.json"
        $analysisBacklink = Read-JsonFile $analysisPath
        $analysisBacklink.artifact_backlink.root_manifest = ConvertTo-RepositoryRelativePath -RepositoryRoot $RepositoryRoot -Path $manifestPath
        $analysisBacklink | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $analysisPath -Encoding UTF8
        $analysisLink = New-UiAuditRepositoryArtifactLink -RepositoryRoot $RepositoryRoot -Path $analysisPath
        $bundle = [ordered]@{
            schema_version = 1
            algorithm_version = "ui_comparison_bundle_v1"
            run_id = $RunIdValue
            root_manifest = [ordered]@{ path = ConvertTo-RepositoryRelativePath -RepositoryRoot $RepositoryRoot -Path $manifestPath }
            analysis = $analysisLink
            fix_iterations = @()
            captures = @($captureResults.ToArray())
        }
        Write-UiAuditJsonFile -Path $bundlePath -Value $bundle
        $comparisonInputLink = New-UiAuditRepositoryArtifactLink -RepositoryRoot $RepositoryRoot -Path $bundlePath
        $manifest | Add-Member -NotePropertyName "runner_analysis" -NotePropertyValue $manifest.analysis -Force
        $manifest | Add-Member -NotePropertyName "analysis" -NotePropertyValue $analysisLink -Force
        $manifest | Add-Member -NotePropertyName "fix_iterations" -NotePropertyValue @() -Force
        $manifest | Add-Member -NotePropertyName "comparison" -NotePropertyValue ([ordered]@{
                mode = "strict_reference"
                status = "building_report"
                reference_manifest = $referenceLink
                input = [string]$comparisonInputLink.path
                input_sha256 = [string]$comparisonInputLink.sha256
                matrix_total = $references.Count
                ai_mode = $ComparisonAiMode.ToLowerInvariant()
            }) -Force
        $manifest | ConvertTo-Json -Depth 40 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
        try {
            $reportOutput = Join-FullPath $RunRoot "comparison/report"
            [void](Invoke-UiAuditVisualTool -RepositoryRoot $RepositoryRoot -RunRoot $RunRoot -Command "build-report" -LogName "comparison-report" -Arguments ((Get-UiAuditComparisonArguments -RepositoryRoot $RepositoryRoot -RunRoot $RunRoot) + @("--bundle", $bundlePath, "--output-directory", $reportOutput)))
            $resultPath = Join-FullPath $reportOutput "comparison-result.json"
            $reportPath = Join-FullPath $reportOutput "report.md"
            $resultLink = New-UiAuditRepositoryArtifactLink -RepositoryRoot $RepositoryRoot -Path $resultPath
            $reportLink = New-UiAuditRepositoryArtifactLink -RepositoryRoot $RepositoryRoot -Path $reportPath
        } catch {
            $failures.Add([pscustomobject]@{ screen = "matrix"; device = "all"; state = "all"; reason = "build-report failed: $($_.Exception.Message)" })
        }
    }
    $gateFailures = @($captureResults.ToArray() | Where-Object { $_.gate_state -ne "passed" } | ForEach-Object {
            [pscustomobject]@{ screen = $_.screen; device = $_.device; state = $_.state; reason = "visual gate state $($_.gate_state)" }
        })
    foreach ($failure in $gateFailures) {
        $failures.Add($failure)
    }
    $status = if ($failures.Count -gt 0) { "failed" } else { "passed" }
    $comparisonSummary = [ordered]@{
        mode = "strict_reference"
        status = $status
        reference_manifest = $referenceLink
        input = if ($null -eq $comparisonInputLink) { $null } else { [string]$comparisonInputLink.path }
        input_sha256 = if ($null -eq $comparisonInputLink) { $null } else { [string]$comparisonInputLink.sha256 }
        result = $resultLink
        report = $reportLink
        matrix_total = $references.Count
        compared_captures = $captureResults.Count
        failed_captures = @($failures.ToArray())
        ai_mode = $ComparisonAiMode.ToLowerInvariant()
        performance = [ordered]@{
            matrix_elapsed_milliseconds = [int64]$overall.ElapsedMilliseconds
            estimated_peak_memory_bytes = $peakMemory
            artifact_bytes = $artifactBytes
            budgets = [ordered]@{
                matrix_seconds = $ComparisonBudgetSeconds
                peak_memory_bytes = $ComparisonBudgetPeakMemoryBytes
                artifact_bytes = $ComparisonBudgetArtifactBytes
            }
        }
    }
    $manifest | Add-Member -NotePropertyName "comparison" -NotePropertyValue $comparisonSummary -Force
    if ($status -eq "failed") {
        $manifest.status = "failed"
    }
    $manifest | ConvertTo-Json -Depth 40 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
    Add-Content -LiteralPath (Join-FullPath $RunRoot "report.md") -Encoding UTF8 -Value ("`n## Strict Reference Comparison`n`n- Status: ``$status```n- Matrix elapsed: ``$([int64]$overall.ElapsedMilliseconds) ms```n- Estimated peak memory: ``$peakMemory bytes```n- Artifact bytes: ``$artifactBytes```n- Full report: $(if ($reportLink) { "[$($reportLink.path)]($($reportLink.path))" } else { "-" })`n")
    Write-Host "Strict comparison: $status; captures=$($captureResults.Count)/$($references.Count); elapsed=$([int64]$overall.ElapsedMilliseconds)ms; artifacts=$artifactBytes bytes"
    return $comparisonSummary
}

function Write-UiAuditStrictSelfTestPng {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][int]$Width,
        [Parameter(Mandatory = $true)][int]$Height,
        [Parameter(Mandatory = $true)][System.Drawing.Color]$Color
    )

    Add-Type -AssemblyName System.Drawing
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    $bitmap = New-Object System.Drawing.Bitmap $Width, $Height
    try {
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.Clear($Color)
            $accent = [System.Drawing.Color]::FromArgb(255, 255 - $Color.R, 255 - $Color.G, 255 - $Color.B)
            $accentBrush = New-Object System.Drawing.SolidBrush $accent
            $lightBrush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::White)
            try {
                $graphics.FillRectangle($accentBrush, [int]($Width * 0.08), [int]($Height * 0.08), [int]($Width * 0.84), [int]($Height * 0.12))
                $graphics.FillEllipse($lightBrush, [int]($Width * 0.35), [int]($Height * 0.38), [int]($Width * 0.3), [int]($Height * 0.3))
            } finally {
                $accentBrush.Dispose()
                $lightBrush.Dispose()
            }
        } finally {
            $graphics.Dispose()
        }
        $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $bitmap.Dispose()
    }
}

function Invoke-UiAuditStrictReferenceSelfTest {
    param([Parameter(Mandatory = $true)][string]$RepositoryRoot)

    $suffix = "strict-selftest-" + [Guid]::NewGuid().ToString("N")
    $runRoot = Join-FullPath $RepositoryRoot (Join-Path "summary/ui-audit" $suffix)
    $referenceRoot = Join-FullPath $RepositoryRoot (Join-Path "summary/ui-visual-audit" $suffix)
    $manifestPath = Join-FullPath $runRoot "reference-manifest.json"
    $savedReferenceManifest = $ReferenceManifest
    $savedStrictReference = $StrictReference
    $savedDeterministicCapture = $DeterministicCapture
    $savedComparisonAiMode = $ComparisonAiMode
    try {
        $profiles = @(
            [pscustomobject]@{ device = "phone-small"; state = "initial"; color = [System.Drawing.Color]::SteelBlue },
            [pscustomobject]@{ device = "phone-small"; state = "bottom"; color = [System.Drawing.Color]::DarkSlateBlue },
            [pscustomobject]@{ device = "tablet-portrait"; state = "initial"; color = [System.Drawing.Color]::DarkOliveGreen }
        )
        $references = New-Object System.Collections.Generic.List[object]
        $results = New-Object System.Collections.Generic.List[object]
        foreach ($profile in $profiles) {
            $deviceProfile = Get-UiAuditDeterministicProfile -Device $profile.device
            $fileName = "$($profile.device)-$($profile.state).png"
            $referenceImage = Join-FullPath $referenceRoot $fileName
            $actualImage = Join-FullPath $runRoot (Join-Path "captures" $fileName)
            Write-UiAuditStrictSelfTestPng -Path $referenceImage -Width $deviceProfile.physical_width -Height $deviceProfile.physical_height -Color $profile.color
            New-Item -ItemType Directory -Force -Path (Split-Path -Parent $actualImage) | Out-Null
            Copy-Item -LiteralPath $referenceImage -Destination $actualImage -Force
            $metadata = Join-FullPath $runRoot (Join-Path "captures" "$($profile.device).metadata.json")
            Copy-Item -LiteralPath (Join-FullPath $RepositoryRoot "tools/ui-visual-audit/fixtures/semantic/compact-pass.metadata.json") -Destination $metadata -Force
            $hash = (Get-FileHash -LiteralPath $referenceImage -Algorithm SHA256).Hash.ToLowerInvariant()
            $references.Add([ordered]@{
                    reference_id = "selftest_ui_gallery_$($profile.device)_$($profile.state)"
                    key = [ordered]@{ screen = "ui_gallery"; device = $profile.device; state = $profile.state; locale = "zh_cn"; theme = "default" }
                    viewport = [ordered]@{
                        logical_size = [ordered]@{ width = [double]$deviceProfile.logical_width; height = [double]$deviceProfile.logical_height }
                        physical_size = [ordered]@{ width = [int]$deviceProfile.physical_width; height = [int]$deviceProfile.physical_height }
                        device_scale = [double]$deviceProfile.device_scale
                        orientation = "portrait"
                    }
                    image = [ordered]@{ storage = "temporary_local"; relative_path = "$suffix/$fileName"; sha256 = $hash }
                    metadata = [ordered]@{ original_size = [ordered]@{ width = [int]$deviceProfile.physical_width; height = [int]$deviceProfile.physical_height }; color_space = "srgb" }
                    provenance = [ordered]@{ source = "runner strict self-test"; source_uri = $null; authorization_status = "local_restricted"; license_id = "test-only" }
                    baseline = [ordered]@{ version = 1; update_reason = "runner strict self-test"; previous_sha256 = $null }
                    allowed_differences = [ordered]@{ profile = "strict_selftest"; per_channel_tolerance = 0; max_changed_pixel_ratio = 0.0; notes = @("generated only for runner self-test") }
                })
            $results.Add([pscustomobject]@{
                    screen = "ui_gallery"
                    device = $profile.device
                    status = "passed"
                    captures = @([pscustomobject]@{
                            screen = "ui_gallery"
                            device = $profile.device
                            state = $profile.state
                            status = "passed"
                            screenshot = ConvertTo-RunRelativePath -RunRoot $runRoot -Path $actualImage
                            metadata = ConvertTo-RunRelativePath -RunRoot $runRoot -Path $metadata
                            screenshot_exists = $true
                            metadata_exists = $true
                            repetition_index = 1
                        })
                })
        }
        Write-UiAuditJsonFile -Path $manifestPath -Value ([ordered]@{ schema_version = 1; references = @($references.ToArray()) })
        $ReferenceManifest = $manifestPath
        $StrictReference = $true
        $DeterministicCapture = $true
        $ComparisonAiMode = "Fixture"
        Write-UiAuditRunnerOutputs -RunRoot $runRoot -RunIdValue $suffix -Results @($results.ToArray()) -ScreensValue @("ui_gallery") -DevicesValue @($profiles | ForEach-Object { $_.device }) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($profiles | ForEach-Object { $_.device })
        $comparison = Complete-UiAuditReferenceComparison -RepositoryRoot $RepositoryRoot -RunRoot $runRoot -RunIdValue $suffix -Results @($results.ToArray()) -IsDryRun $false
        Assert-SelfTest ($comparison.status -eq "passed" -and $comparison.compared_captures -eq 3) "strict reference comparison expands and passes three capture mappings"
        Assert-SelfTest ((Test-Path (Join-FullPath $runRoot "comparison/report/comparison-result.json")) -and (Test-Path (Join-FullPath $runRoot "comparison/report/report.md"))) "strict reference comparison writes structured result and report"
        $strictResult = Read-JsonFile (Join-FullPath $runRoot "comparison/report/comparison-result.json")
        Assert-SelfTest (@($strictResult.captures).Count -eq 3 -and @($strictResult.captures | ForEach-Object { $_.device } | Select-Object -Unique).Count -eq 2 -and @($strictResult.captures | ForEach-Object { $_.state } | Select-Object -Unique).Count -eq 2) "strict reference comparison covers two devices and a multi-state UI page"
        $strictManifest = Read-JsonFile (Join-FullPath $runRoot "manifest.json")
        Assert-SelfTest ($strictManifest.comparison.performance.matrix_elapsed_milliseconds -ge 0 -and $strictManifest.comparison.performance.artifact_bytes -gt 0) "strict comparison records matrix time and artifact budget evidence"
    } finally {
        $ReferenceManifest = $savedReferenceManifest
        $StrictReference = $savedStrictReference
        $DeterministicCapture = $savedDeterministicCapture
        $ComparisonAiMode = $savedComparisonAiMode
        if (Test-Path -LiteralPath $runRoot) {
            Remove-Item -LiteralPath $runRoot -Recurse -Force
        }
        if (Test-Path -LiteralPath $referenceRoot) {
            Remove-Item -LiteralPath $referenceRoot -Recurse -Force
        }
    }
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
    $failedComparisons = @()
    if ($null -ne $manifest.PSObject.Properties["comparison"] -and $null -ne $manifest.comparison -and $null -ne $manifest.comparison.PSObject.Properties["failed_captures"]) {
        $failedComparisons = @($manifest.comparison.failed_captures | Where-Object {
                -not [string]::IsNullOrWhiteSpace([string]$_.screen) -and
                    -not [string]::IsNullOrWhiteSpace([string]$_.device) -and
                    -not [string]::IsNullOrWhiteSpace([string]$_.state)
            })
    }
    if ($failed.Count -eq 0 -and $failedComparisons.Count -eq 0) {
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
            $seeds.Add([pscustomobject]@{ screen = $screen; device = $device; states = $null })
        }
        foreach ($capture in $failedComparisons) {
            $seeds.Add([pscustomobject]@{
                    screen = [string]$capture.screen
                    device = [string]$capture.device
                    states = [string]$capture.state
                })
        }
    } else {
        $screens = @($failed | ForEach-Object { [string]$_.screen } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Unique)
        foreach ($screen in $screens) {
            foreach ($device in $MatrixDevices) {
                $seeds.Add([pscustomobject]@{ screen = $screen; device = $device; states = $null })
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
        $seedStates = if ($null -ne $seed.PSObject.Properties["states"] -and -not [string]::IsNullOrWhiteSpace([string]$seed.states)) { [string]$seed.states } else { $StateValue }
        $tasks.Add((New-UiAuditTask -RunRoot $RunRoot -Screen ([string]$seed.screen) -Device ([string]$seed.device) -StateValue $seedStates -ExtraBevyArgs $ExtraBevyArgs))
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
    $psi.Environment["MYBEVY_UI_AUDIT_GIT_COMMIT"] = Get-UiAuditGitCommit -RepositoryRoot (Split-Path -Parent $ProjectRoot)
    $psi.Environment["MYBEVY_UI_AUDIT_DETERMINISTIC"] = if ($null -ne $Task.determinism) { "1" } else { "0" }
    if ($null -ne $Task.determinism) {
        $target = $Task.determinism.target_viewport
        $psi.Environment["MYBEVY_UI_AUDIT_TARGET_LOGICAL_WIDTH"] = ([double]$target.logical_width).ToString([Globalization.CultureInfo]::InvariantCulture)
        $psi.Environment["MYBEVY_UI_AUDIT_TARGET_LOGICAL_HEIGHT"] = ([double]$target.logical_height).ToString([Globalization.CultureInfo]::InvariantCulture)
        $psi.Environment["MYBEVY_UI_AUDIT_TARGET_PHYSICAL_WIDTH"] = ([int]$target.physical_width).ToString([Globalization.CultureInfo]::InvariantCulture)
        $psi.Environment["MYBEVY_UI_AUDIT_TARGET_PHYSICAL_HEIGHT"] = ([int]$target.physical_height).ToString([Globalization.CultureInfo]::InvariantCulture)
        $psi.Environment["MYBEVY_UI_AUDIT_TARGET_DEVICE_SCALE"] = ([double]$target.device_scale).ToString([Globalization.CultureInfo]::InvariantCulture)
        $psi.Environment["MYBEVY_UI_AUDIT_LOCALE"] = [string]$Task.determinism.locale
        $psi.Environment["MYBEVY_UI_AUDIT_THEME"] = [string]$Task.determinism.theme
        $psi.Environment["MYBEVY_UI_AUDIT_RANDOM_SEED"] = ([UInt64]$Task.determinism.random_seed).ToString([Globalization.CultureInfo]::InvariantCulture)
        $psi.Environment["MYBEVY_UI_AUDIT_FROZEN_TIME_SECONDS"] = ([double]$Task.determinism.frozen_time_seconds).ToString([Globalization.CultureInfo]::InvariantCulture)
        $psi.Environment["MYBEVY_UI_AUDIT_ANIMATION_PROGRESS"] = "1"
        $psi.Environment["MYBEVY_UI_AUDIT_DYNAMIC_POLICY"] = [string]$Task.determinism.dynamic_policy
        [void]$psi.Environment.Remove("MYBEVY_UI_AUDIT_STABLE_FIXTURE_ID")
        [void]$psi.Environment.Remove("MYBEVY_UI_AUDIT_DYNAMIC_MASK_ID")
        if (-not [string]::IsNullOrWhiteSpace([string]$Task.determinism.stable_fixture_id)) {
            $psi.Environment["MYBEVY_UI_AUDIT_STABLE_FIXTURE_ID"] = [string]$Task.determinism.stable_fixture_id
        }
        if (-not [string]::IsNullOrWhiteSpace([string]$Task.determinism.dynamic_mask_id)) {
            $psi.Environment["MYBEVY_UI_AUDIT_DYNAMIC_MASK_ID"] = [string]$Task.determinism.dynamic_mask_id
        }
        $psi.Environment["MYBEVY_UI_AUDIT_REPEAT_CAPTURES"] = ([int]$Task.determinism.repeat_captures).ToString([Globalization.CultureInfo]::InvariantCulture)
        $psi.Environment["MYBEVY_UI_LOCALE"] = [string]$Task.determinism.locale
        if ([string]$Task.determinism.theme -eq "default") {
            [void]$psi.Environment.Remove("MYBEVY_UI_THEME")
        } else {
            $psi.Environment["MYBEVY_UI_THEME"] = [string]$Task.determinism.theme
        }
    }

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

function Invoke-UiAuditClosedLoopGeneration {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$Mode
    )

    if ($Mode -eq "Off") {
        return $null
    }
    if ([string]::IsNullOrWhiteSpace($GenerationDocumentId)) {
        throw "-GenerationDocumentId must not be empty when -GenerationMode is enabled."
    }

    if ([string]::IsNullOrWhiteSpace($GenerationTask)) {
        throw "-GenerationTask is required when -GenerationMode is enabled; use a task with a unique run_id and approved reference metadata."
    }
    $taskPath = Get-FullPath $GenerationTask
    if (-not (Test-Path -LiteralPath $taskPath -PathType Leaf)) {
        throw "Generation task was not found: $taskPath"
    }

    $arguments = @(
        "run", "--quiet",
        "--manifest-path", (Join-FullPath $RepositoryRoot "tools/ui-generation/Cargo.toml"),
        "--",
        "closed-loop-generate",
        "--mode", $Mode.ToLowerInvariant(),
        "--task", $taskPath,
        "--repository-root", $RepositoryRoot,
        "--document-id", $GenerationDocumentId.Trim()
    )
    if (-not [string]::IsNullOrWhiteSpace($GenerationOptions)) {
        $arguments += @("--options", (Get-FullPath $GenerationOptions))
    }
    if (-not [string]::IsNullOrWhiteSpace($ProviderCredentialEnvironment)) {
        $arguments += @("--provider-credential-environment", $ProviderCredentialEnvironment.Trim())
    }

    # Prompt construction, model output validation, credential resolution, and provider metadata
    # remain inside the stable Rust tool contract. This wrapper only records its JSON result.
    $hadSourceCommit = Test-Path "Env:MYBEVY_UI_AUDIT_GIT_COMMIT"
    $previousSourceCommit = $env:MYBEVY_UI_AUDIT_GIT_COMMIT
    $env:MYBEVY_UI_AUDIT_GIT_COMMIT = Get-UiAuditGitCommit -RepositoryRoot $RepositoryRoot
    try {
        $output = @(& cargo @arguments 2>&1)
        $exitCode = $LASTEXITCODE
    } finally {
        if ($hadSourceCommit) {
            $env:MYBEVY_UI_AUDIT_GIT_COMMIT = $previousSourceCommit
        } else {
            Remove-Item "Env:MYBEVY_UI_AUDIT_GIT_COMMIT" -ErrorAction SilentlyContinue
        }
    }
    if ($exitCode -ne 0) {
        $detail = ($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine
        throw "Closed-loop generation mode $Mode failed: $detail"
    }
    try {
        return (($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine) | ConvertFrom-Json -ErrorAction Stop
    } catch {
        throw "Closed-loop generation tool did not emit a valid JSON result: $($_.Exception.Message)"
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
            repetition_index = if ($null -ne $entry.PSObject.Properties["repetition_index"]) { [int]$entry.repetition_index } else { 1 }
            repetition_total = if ($null -ne $entry.PSObject.Properties["repetition_total"]) { [int]$entry.repetition_total } else { 1 }
            screenshot_sha256 = if ($null -ne $entry.PSObject.Properties["screenshot_sha256"]) { [string]$entry.screenshot_sha256 } else { $null }
            screenshot_byte_length = if ($null -ne $entry.PSObject.Properties["screenshot_byte_length"]) { [UInt64]$entry.screenshot_byte_length } else { $null }
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
        determinism = $Task.determinism
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
        $deterministicFailureTypes = @("document_not_ready", "font_not_ready", "image_not_ready", "unstable_ui", "screenshot_size_mismatch", "nondeterministic_capture")
        $firstFailure = [string]$failedCaptures[0].failure
        $base.failure_type = if ($firstFailure -in $deterministicFailureTypes) { $firstFailure } else { "audit_failed" }
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
        determinism = $Task.determinism
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
        [string[]]$LocalDevicesValue = @(),
        [ValidateSet("Auto", "Fixture", "Provider", "Off")]
        [string]$AnalysisModeName = $AnalysisMode,
        [AllowEmptyString()][string]$AnalysisResultFile = $AnalysisResultPath
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
        deterministic_capture = [bool]$DeterministicCapture
        deterministic_capture_config = if ($DeterministicCapture) {
            [ordered]@{
                locale = $Locale.Trim().ToLowerInvariant().Replace("-", "_")
                theme = $Theme.Trim()
                random_seed = $RandomSeed
                frozen_time_seconds = $FrozenTimeSeconds
                animation_progress = 1.0
                dynamic_policy = if ($DynamicContentPolicy -eq "StableFixture") { "stable_fixture" } else { "explicit_mask" }
                stable_fixture_id = if ($DynamicContentPolicy -eq "StableFixture") { $StableFixtureId.Trim() } else { $null }
                dynamic_mask_id = if ($DynamicContentPolicy -eq "ExplicitMask") { $DynamicMaskId.Trim() } else { $null }
                repeat_captures = $RepeatCaptures
                comparison = "exact_png_sha256"
            }
        } else { $null }
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
    if ($null -ne $script:UiAuditClosedLoopGeneration) {
        $manifest.generation = $script:UiAuditClosedLoopGeneration
    }

    $manifestPath = Join-FullPath $RunRoot "manifest.json"
    $reportPath = Join-FullPath $RunRoot "report.md"
    $manifest | ConvertTo-Json -Depth 20 | Set-Content -Path $manifestPath -Encoding UTF8
    $analysisInput = New-UiAuditAnalysisInput -RunRoot $RunRoot -Manifest ([pscustomobject]$manifest)
    $analysisInputPath = Write-UiAuditAnalysisInput -RunRoot $RunRoot -AnalysisInput $analysisInput
    $analysisResultFullPath = if ([string]::IsNullOrWhiteSpace($AnalysisResultFile)) { "" } else { Get-FullPath $AnalysisResultFile }
    $analysis = Invoke-UiAuditAnalysis -RunRoot $RunRoot -AnalysisInput $analysisInput -InputPath $analysisInputPath -Mode $AnalysisModeName -ResultPath $analysisResultFullPath
    $analysisPath = Write-UiAuditAnalysisOutput -RunRoot $RunRoot -Analysis $analysis -RunIdValue $RunIdValue -CaptureIds @($analysisInput.captures | ForEach-Object { [string]$_.capture_id })
    $analysisForManifest = [ordered]@{
        input = $analysisInputPath
        output = $analysisPath
        mode = $analysis.mode
        status = $analysis.status
        pass = [bool]$analysis.pass
        failure_type = $analysis.failure_type
        detail = $analysis.detail
        severity_counts = $analysis.severity_counts
        issues = @($analysis.issues)
    }
    if ($AnalysisModeName -eq "Provider") {
        $analysisForManifest.deterministic_hard_failure_count = [int]$analysis.deterministic_hard_failure_count
        $analysisForManifest.deterministic_hard_failures = @($analysis.deterministic_hard_failures)
    }
    $manifest.analysis = $analysisForManifest
    if ($status -eq "passed" -and $analysis.status -eq "failed") {
        $manifest.status = "failed"
    }
    $script:LastUiAuditAnalysisStatus = $analysis
    $manifest.artifact_links = [ordered]@{
        schema_version = 1
        analysis_input = New-UiAuditArtifactLink -RunRoot $RunRoot -Path (Join-FullPath $RunRoot $analysisInputPath)
        analysis = New-UiAuditArtifactLink -RunRoot $RunRoot -Path (Join-FullPath $RunRoot $analysisPath)
        comparison = $null
        fix_iterations = @()
    }
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

function Format-MarkdownTableText {
    param([AllowNull()][string]$Value)

    if ([string]::IsNullOrWhiteSpace($Value)) {
        return "-"
    }

    return (($Value -replace "\r?\n", "<br>") -replace "\|", "\|")
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

function Format-SnapshotReference {
    param(
        [AllowNull()][string]$Path,
        [string]$FileName = "snapshot.json"
    )

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return "-"
    }

    $normalized = ($Path -replace "\\", "/").TrimEnd("/")
    if ([string]::IsNullOrWhiteSpace($normalized)) {
        return "-"
    }

    return Format-MarkdownLink "snapshot" "$normalized/$FileName"
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
    $lines.Add("- Deterministic capture: ``$([bool]$Manifest.deterministic_capture)``")
    if ([bool]$Manifest.deterministic_capture) {
        $lines.Add("- Repetitions: ``$($Manifest.deterministic_capture_config.repeat_captures)`` (exact PNG SHA-256)")
        $lines.Add("- Locale / theme: ``$($Manifest.deterministic_capture_config.locale)`` / ``$($Manifest.deterministic_capture_config.theme)``")
    }
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

    if ($null -ne $Manifest.analysis) {
        $lines.Add("")
        $lines.Add("## Analysis")
        $lines.Add("")
        $lines.Add("- Mode: ``$($Manifest.analysis.mode)``")
        $lines.Add("- Status: ``$($Manifest.analysis.status)``")
        $lines.Add("- Pass: ``$($Manifest.analysis.pass)``")
        if ($Manifest.analysis.failure_type) {
            $lines.Add("- Failure: ``$($Manifest.analysis.failure_type)``")
        }
        if ($Manifest.analysis.detail) {
            $lines.Add("- Detail: $(Format-MarkdownTableText $Manifest.analysis.detail)")
        }
        $lines.Add("- Input: $(Format-MarkdownLink "analysis-input.json" $Manifest.analysis.input)")
        $lines.Add("- Output: $(Format-MarkdownLink "analysis.json" $Manifest.analysis.output)")
        if ($null -ne $Manifest.analysis.severity_counts) {
            $counts = $Manifest.analysis.severity_counts
            $lines.Add("- Severity counts: severe=$($counts.severe), medium=$($counts.medium), minor=$($counts.minor), blocking=$($counts.blocking)")
        }

        $issues = @($Manifest.analysis.issues)
        if ($issues.Count -gt 0) {
            $captureByKey = @{}
            foreach ($capture in $allCaptures) {
                $key = "$($capture.screen)|$($capture.device)|$($capture.state)"
                if (-not $captureByKey.ContainsKey($key)) {
                    $captureByKey[$key] = $capture
                }
            }

            $lines.Add("")
            $lines.Add("| Screen | Device | State | Severity | Blocking | Screenshot | Metadata | Problem | Evidence | Likely cause | Suggested files |")
            $lines.Add("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
            foreach ($issue in $issues) {
                $key = "$($issue.screen)|$($issue.device)|$($issue.state)"
                $capture = if ($captureByKey.ContainsKey($key)) { $captureByKey[$key] } else { $null }
                $screenshot = if ($null -ne $capture) {
                    Format-ArtifactReference -Path $capture.screenshot -Uri $capture.screenshot_artifact_uri -Label "screenshot"
                } else {
                    "-"
                }
                $metadata = if ($null -ne $capture) {
                    Format-ArtifactReference -Path $capture.metadata -Uri $capture.metadata_artifact_uri -Label "metadata"
                } else {
                    "-"
                }
                $suggested = @($issue.suggested_files) -join "<br>"
                $lines.Add("| ``$($issue.screen)`` | ``$($issue.device)`` | ``$($issue.state)`` | ``$($issue.severity)`` | ``$($issue.blocking)`` | $screenshot | $metadata | $(Format-MarkdownTableText $issue.problem) | $(Format-MarkdownTableText $issue.evidence) | $(Format-MarkdownTableText $issue.likely_cause) | $(Format-MarkdownTableText $suggested) |")
            }
        } else {
            $lines.Add("")
            $lines.Add("No analysis issues.")
        }
    }

    if ($null -ne $Manifest.PSObject.Properties["artifact_links"] -and $null -ne $Manifest.artifact_links) {
        $links = $Manifest.artifact_links
        $lines.Add("")
        $lines.Add("## Artifact Links")
        $lines.Add("")
        $lines.Add("- Analysis input: $(Format-MarkdownLink "analysis-input" $links.analysis_input.path) (``$($links.analysis_input.sha256)``)")
        $lines.Add("- Analysis output: $(Format-MarkdownLink "analysis" $links.analysis.path) (``$($links.analysis.sha256)``)")
        if ($links.comparison) {
            $lines.Add("- Comparison: $(Format-MarkdownLink "comparison" $links.comparison.path) (``$($links.comparison.sha256)``)")
        } else {
            $lines.Add("- Comparison: pending strict comparison bundle; Stage 11 runner integration is not implied")
        }
        $linkedIterations = @($links.fix_iterations)
        if ($linkedIterations.Count -gt 0) {
            $lines.Add("")
            $lines.Add("| Fix iteration | Manifest | Analysis | Report |")
            $lines.Add("| --- | --- | --- | --- |")
            foreach ($link in $linkedIterations) {
                $linkedManifest = Format-MarkdownLink "manifest" $link.manifest.path
                $linkedAnalysis = if ($link.analysis) { Format-MarkdownLink "analysis" $link.analysis.path } else { "-" }
                $linkedReport = if ($link.report) { Format-MarkdownLink "report" $link.report.path } else { "-" }
                $lines.Add("| $($link.iteration) | $linkedManifest | $linkedAnalysis | $linkedReport |")
            }
        }
    }

    if ($null -ne $Manifest.PSObject.Properties["fix_loop"] -and $null -ne $Manifest.fix_loop) {
        $fix = $Manifest.fix_loop
        $lines.Add("")
        $lines.Add("## Fix Loop")
        $lines.Add("")
        $lines.Add("- Mode: ``$($fix.mode)``")
        $lines.Add("- Status: ``$($fix.status)``")
        $lines.Add("- Pass: ``$($fix.pass)``")
        $lines.Add("- Max iterations: $($fix.max_fix_iterations)")
        if ($fix.failure_type) {
            $lines.Add("- Failure: ``$($fix.failure_type)``")
        }
        if ($fix.detail) {
            $lines.Add("- Detail: $(Format-MarkdownTableText $fix.detail)")
        }
        if ($fix.before) {
            $beforePath = if ($fix.before.PSObject.Properties["path"]) { [string]$fix.before.path } else { "" }
            $lines.Add("- Before snapshot: $(Format-SnapshotReference -Path $beforePath)")
        }
        if ($fix.plan) {
            $lines.Add("- Plan: $(Format-MarkdownLink "fix-plan.json" $fix.plan)")
        }
        if ($fix.safety_policy) {
            $lines.Add("- Allowed roots: ``$(@($fix.safety_policy.allowed_roots) -join ', ')``")
        }

        if ($fix.strategy_priority) {
            $lines.Add("")
            $lines.Add("| Priority | Scope | Allowed roots |")
            $lines.Add("| --- | --- | --- |")
            foreach ($strategy in @($fix.strategy_priority)) {
                $lines.Add("| ``$($strategy.id)`` | $(Format-MarkdownTableText $strategy.label) | ``$(@($strategy.allowed_roots) -join ', ')`` |")
            }
        }

        $iterations = @($fix.iterations)
        if ($iterations.Count -gt 0) {
            $lines.Add("")
            $lines.Add("| Iteration | Status | Failure | Plan | Cargo logs | After report | After snapshot |")
            $lines.Add("| --- | --- | --- | --- | --- | --- | --- |")
            foreach ($iteration in $iterations) {
                $failure = if ($iteration.failure_type) { "``$($iteration.failure_type)``" } else { "-" }
                $plan = Format-MarkdownLink "plan" $iteration.fix_plan
                $afterReport = Format-MarkdownLink "after report" $iteration.after_report
                $afterSnapshot = if ($iteration.after_snapshot) {
                    $afterPath = if ($iteration.after_snapshot.PSObject.Properties["path"]) { [string]$iteration.after_snapshot.path } else { "" }
                    Format-SnapshotReference -Path $afterPath
                } else {
                    "-"
                }
                $cargoLogs = "-"
                if ($iteration.checks -and $iteration.checks.commands) {
                    $cargoLogs = ((@($iteration.checks.commands) | ForEach-Object {
                                "$($_.command): $(Format-MarkdownLink "stdout" $_.stdout)/$(Format-MarkdownLink "stderr" $_.stderr)"
                            }) -join "<br>")
                }
                $lines.Add("| $($iteration.iteration) | ``$($iteration.status)`` | $failure | $plan | $cargoLogs | $afterReport | $afterSnapshot |")
            }
        }

        $finalIssues = @($fix.final_issues)
        if ($finalIssues.Count -gt 0) {
            $lines.Add("")
            $lines.Add("Remaining blocking issues:")
            $lines.Add("")
            $lines.Add("| Screen | Device | State | Severity | Problem | Suggested files |")
            $lines.Add("| --- | --- | --- | --- | --- | --- |")
            foreach ($issue in $finalIssues) {
                $suggested = @($issue.suggested_files) -join "<br>"
                $lines.Add("| ``$($issue.screen)`` | ``$($issue.device)`` | ``$($issue.state)`` | ``$($issue.severity)`` | $(Format-MarkdownTableText $issue.problem) | $(Format-MarkdownTableText $suggested) |")
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
        repetition_index = 1
        repetition_total = 1
        screenshot_sha256 = $null
        screenshot_byte_length = $null
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

function Write-FakeAnalysisResult {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [AllowEmptyCollection()][object[]]$Issues
    )

    $result = [ordered]@{
        schema_version = 1
        issues = @($Issues)
    }
    $result | ConvertTo-Json -Depth 20 | Set-Content -Path $Path -Encoding UTF8
}

function New-FakeAnalysisIssue {
    param(
        [Parameter(Mandatory = $true)][object]$Capture,
        [string]$Severity = "minor",
        [string]$ProblemType = "visual_polish",
        [string]$Problem = "alignment could be cleaner",
        [AllowNull()][object]$Blocking = $null
    )

    $issue = [ordered]@{
        screen = [string]$Capture.screen
        device = [string]$Capture.device
        state = [string]$Capture.state
        severity = $Severity
        problem_type = $ProblemType
        problem = $Problem
        evidence = "fixture evidence for $($Capture.state)"
        likely_cause = "fixture likely cause"
        suggested_files = @("project/src/game/screens/dev/ui_gallery.rs")
    }
    if ($null -ne $Blocking) {
        $issue.blocking = $Blocking
    }
    return $issue
}

function New-FakePassedUiAuditResult {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [string]$Screen = "ui_gallery",
        [string]$Device = "phone-small"
    )

    $task = @(New-UiAuditTasks -RunRoot $RunRoot -ScreensToRun @($Screen) -DevicesToRun @($Device) -StateValue "initial" -ExtraBevyArgs @())[0]
    New-FakeChildManifest -Task $task -Status "passed" -CreateArtifacts
    $launch = [pscustomobject]@{ started = $true; launch_error = $null; timed_out = $false; exit_code = 0 }
    return Resolve-UiAuditTaskResult -Task $task -LaunchResult $launch -RunRoot $RunRoot
}

function Invoke-UiAuditSelfTest {
    $tempRoot = Join-FullPath ([System.IO.Path]::GetTempPath()) ("mybevy-ui-audit-selftest-" + [Guid]::NewGuid().ToString("N"))
    try {
        $scriptRoot = if (-not [string]::IsNullOrWhiteSpace($PSScriptRoot)) {
            $PSScriptRoot
        } else {
            Split-Path -Parent $PSCommandPath
        }
        $repoRoot = Get-FullPath (Join-Path $scriptRoot "..")
        $projectRoot = Join-FullPath $repoRoot "project"

        $screens = @(Resolve-UiAuditScreens @("ui-gallery,lobby"))
        Assert-SelfTest ($screens.Count -eq 2 -and $screens[0] -eq "ui_gallery" -and $screens[1] -eq "lobby") "screen parsing and alias normalization"

        $devices = @(Resolve-UiAuditDevices @("phone-small", "tablet-portrait"))
        Assert-SelfTest ($devices.Count -eq 2 -and $devices[0] -eq "phone-small" -and $devices[1] -eq "tablet-portrait") "device parsing"

        $extraArgs = Get-WindowArgumentOverrides -WindowProfileValue "" -WindowSizeValue "1280x2772" -DeviceScaleValue "3.25" -WindowScaleValue "50%" -RawBevyArgs @("--foo", "bar") -RawRemainingArgs @("--window-profile", "desktop")
        Assert-SelfTest (($extraArgs -join "|") -eq "--window-size|1280x2772|--device-scale|3.25|--window-scale|50%|--foo|bar|--window-profile|desktop") "window argument expansion"

        $tasks = @(New-UiAuditTasks -RunRoot $tempRoot -ScreensToRun $screens -DevicesToRun $devices -StateValue "auto" -ExtraBevyArgs $extraArgs)
        Assert-SelfTest ($tasks.Count -eq 4) "task matrix expansion"
        Assert-SelfTest ($tasks[0].states -eq "image_fit,visual_foundation,visual_acceptance,image_modes,image_tiling,image_atlas,typography,typography_overflow,icons,icon_states,style_scopes,effects,animations,components,component_checkboxes,component_toggles,component_segmented,component_overlays,component_tooltip,middle,bottom") "ui_gallery auto states"
        Assert-SelfTest ($tasks[2].states -eq "initial") "non-recipe screen auto states"
        Assert-SelfTest (($tasks[0].bevy_args[0] -eq "--window-profile") -and ($tasks[0].bevy_args[1] -eq "phone-small")) "device window profile mapping"
        Assert-SelfTest (($tasks[0].output_dir -replace "\\", "/").Contains("/runs/ui_gallery/phone-small")) "output path layout"

        $savedDeterministicCapture = $DeterministicCapture
        $savedDynamicContentPolicy = $DynamicContentPolicy
        $savedDynamicMaskId = $DynamicMaskId
        $savedRepeatCaptures = $RepeatCaptures
        $DeterministicCapture = $true
        $DynamicContentPolicy = "StableFixture"
        $RepeatCaptures = 2
        $deterministicTask = New-UiAuditTask -RunRoot (Join-FullPath $tempRoot "deterministic") -Screen "ui_generated_acceptance" -Device "phone-small" -StateValue "initial" -ExtraBevyArgs @()
        Assert-SelfTest ($deterministicTask.determinism.repeat_captures -eq 2 -and $deterministicTask.determinism.dynamic_policy -eq "stable_fixture") "deterministic task carries repetition and closed dynamic policy"
        Assert-SelfTest ($deterministicTask.determinism.target_viewport.logical_width -eq 360.0 -and $deterministicTask.determinism.target_viewport.physical_width -eq 720 -and $deterministicTask.determinism.target_viewport.device_scale -eq 2.0) "deterministic task binds logical physical and scale viewport"
        $DynamicContentPolicy = "ExplicitMask"
        $DynamicMaskId = ""
        $maskRejected = $false
        try {
            [void](New-UiAuditTask -RunRoot (Join-FullPath $tempRoot "missing-mask") -Screen "ui_gallery" -Device "phone-small" -StateValue "initial" -ExtraBevyArgs @())
        } catch {
            $maskRejected = $_.Exception.Message -like "*DynamicMaskId*"
        }
        Assert-SelfTest $maskRejected "explicit dynamic policy fails closed without mask id"
        $DeterministicCapture = $savedDeterministicCapture
        $DynamicContentPolicy = $savedDynamicContentPolicy
        $DynamicMaskId = $savedDynamicMaskId
        $RepeatCaptures = $savedRepeatCaptures

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

        $nondeterministicTask = New-UiAuditTask -RunRoot (Join-FullPath $tempRoot "nondeterministic") -Screen "ui_generated_acceptance" -Device "phone-small" -StateValue "initial" -ExtraBevyArgs @()
        New-FakeChildManifest -Task $nondeterministicTask -Status "failed" -Failure "nondeterministic_capture"
        $nondeterministic = Resolve-UiAuditTaskResult -Task $nondeterministicTask -LaunchResult $passedLaunch -RunRoot $tempRoot
        Assert-SelfTest ($nondeterministic.failure_type -eq "nondeterministic_capture") "deterministic capture failure type is preserved"

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
        Assert-SelfTest (Test-Path (Join-FullPath $tempRoot "analysis-input.json")) "analysis input write"
        Assert-SelfTest (Test-Path (Join-FullPath $tempRoot "analysis.json")) "analysis output write"
        $linkedManifest = Read-JsonFile (Join-FullPath $tempRoot "manifest.json")
        Assert-SelfTest ($linkedManifest.artifact_links.analysis.sha256 -match "^[0-9a-f]{64}$" -and $linkedManifest.artifact_links.analysis_input.sha256 -match "^[0-9a-f]{64}$") "root manifest binds analysis artifacts by path and hash"
        $linkedAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($linkedAnalysis.artifact_backlink.root_run_id -eq "selftest" -and @($linkedAnalysis.artifact_backlink.capture_ids).Count -eq 3) "analysis artifact links back to root run and captures"
        $analysisInput = Read-JsonFile (Join-FullPath $tempRoot "analysis-input.json")
        Assert-SelfTest (@($analysisInput.captures).Count -eq 3) "analysis input captures only resolved captures"
        Assert-SelfTest ((@($analysisInput.captures[0].likely_files) -contains "project/src/game/screens/dev/ui_gallery.rs")) "analysis input likely files"
        $autoAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($autoAnalysis.status -eq "skipped" -and [bool]$autoAnalysis.pass) "analysis auto mode skips without fixture"

        $analysisFixtureRoot = Join-FullPath $tempRoot "analysis-fixtures"
        New-Item -ItemType Directory -Force -Path $analysisFixtureRoot | Out-Null
        $minorResultPath = Join-FullPath $analysisFixtureRoot "minor.json"
        Write-FakeAnalysisResult -Path $minorResultPath -Issues @(
            (New-FakeAnalysisIssue -Capture $passed.captures[0] -Severity "minor" -ProblemType "visual_polish" -Problem "对齐可以更整齐")
        )
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest-minor" -Results @($passed) -ScreensValue @("ui_gallery") -DevicesValue @($devices[0]) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($devices[0]) -AnalysisModeName "Fixture" -AnalysisResultFile $minorResultPath
        $minorAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($minorAnalysis.status -eq "passed" -and [bool]$minorAnalysis.pass -and $minorAnalysis.severity_counts.minor -eq 1 -and $minorAnalysis.severity_counts.blocking -eq 0) "minor analysis does not block"
        $minorReport = Get-Content -Raw -Path (Join-FullPath $tempRoot "report.md")
        Assert-SelfTest ($minorReport.Contains("## Analysis") -and $minorReport.Contains("对齐可以更整齐")) "analysis report includes minor issue"

        $providerResultPath = Join-FullPath $analysisFixtureRoot "provider.json"
        $providerFixtureHash = (("a" * 64) -join "")
        [ordered]@{
            schema_version = 1
            algorithm_version = "ui_ai_visual_analysis_v1"
            status = "completed"
            provider = [ordered]@{
                mode = "fixture"
                provider_id = "fixture-ai"
                audit_model_id = "fixture-audit-v1"
                generation_model_id = "fixture-generation-v1"
                self_review_is_sole_conclusion = $false
                attempts = 1
                input_units = $null
                output_units = $null
            }
            input = [ordered]@{
                bundle_path = "bundle.json"
                bundle_sha256 = $providerFixtureHash
                capture_count = 1
                image_count = 1
                image_bytes = 128
                region_metric_count = 1
                semantic_node_count = 1
                provider_images = @(
                    [ordered]@{
                        image_id = "$($passed.captures[0].screen).$($passed.captures[0].device).$($passed.captures[0].state).overlay"
                        source_sha256 = $providerFixtureHash
                        provider_sha256 = $providerFixtureHash
                        redaction_rect_count = 0
                    }
                )
            }
            issues = @(
                [ordered]@{
                    capture_id = "$($passed.captures[0].screen).$($passed.captures[0].device).$($passed.captures[0].state)"
                    problem_type = "spacing"
                    severity = "minor"
                    problem = "provider spacing evidence"
                    evidence = @(
                        [ordered]@{
                            image_id = "$($passed.captures[0].screen).$($passed.captures[0].device).$($passed.captures[0].state).overlay"
                            description = "overlay evidence"
                        }
                    )
                    region = [ordered]@{ region_id = "content"; bounds = $null }
                    reference_element = "content panel"
                    node_id = "page.content"
                    likely_cause = "fixture provider cause"
                    suggested_files = @("project/src/game/screens/dev/ui_gallery.rs")
                }
            )
            deterministic_hard_failures = @()
            deterministic_hard_failures_preserved = $true
            visual_similarity_is_sole_conclusion = $false
            privacy = [ordered]@{
                credentials_persisted = $false
                image_bytes_persisted = $false
                raw_provider_response_persisted = $false
                prompt_persisted = $false
                sensitive_text_redaction = "fixture redaction"
                provider_redacted_image_count = 0
                provider_redaction_rect_count = 0
                metadata_sensitive_string_count = 0
                response_redaction_count = 0
            }
            artifacts = @([ordered]@{ artifact_type = "ai_analysis_report"; path = "ai-analysis-report.json" })
        } | ConvertTo-Json -Depth 20 | Set-Content -Path $providerResultPath -Encoding UTF8
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest-provider" -Results @($passed) -ScreensValue @("ui_gallery") -DevicesValue @($devices[0]) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($devices[0]) -AnalysisModeName "Provider" -AnalysisResultFile $providerResultPath
        $providerAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($providerAnalysis.status -eq "passed" -and $providerAnalysis.mode -eq "Provider" -and $providerAnalysis.deterministic_hard_failure_count -eq 0 -and @($providerAnalysis.deterministic_hard_failures).Count -eq 0 -and $providerAnalysis.issues[0].capture_id -and @($providerAnalysis.issues[0].evidence_images).Count -eq 1) "provider structured analysis compatibility"

        $providerHardFailurePath = Join-FullPath $analysisFixtureRoot "provider-hard-failure.json"
        $providerHardFailure = Read-JsonFile $providerResultPath
        $providerHardFailure.issues = @()
        $providerHardFailure.deterministic_hard_failures = @(
            [ordered]@{
                capture_id = "$($passed.captures[0].screen).$($passed.captures[0].device).$($passed.captures[0].state)"
                finding = [ordered]@{
                    code = "critical_text_clipped"
                    severity = "hard_failure"
                    message = "critical text is clipped"
                    primary = [ordered]@{
                        stable_id = "root/title"
                        capture_entity = "1v1#fixture"
                        entity_name = "Title"
                        document_id = $null
                        node_id = "title"
                        source_path = "project/src/game/screens/dev/ui_gallery.rs"
                        panel_id = "page"
                        likely_files = @("project/src/game/screens/dev/ui_gallery.rs")
                    }
                }
            }
        )
        $providerHardFailure | ConvertTo-Json -Depth 20 | Set-Content -Path $providerHardFailurePath -Encoding UTF8
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest-provider-hard-failure" -Results @($passed) -ScreensValue @("ui_gallery") -DevicesValue @($devices[0]) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($devices[0]) -AnalysisModeName "Provider" -AnalysisResultFile $providerHardFailurePath
        $providerHardAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        $providerHardManifest = Read-JsonFile (Join-FullPath $tempRoot "manifest.json")
        Assert-SelfTest ($providerHardAnalysis.status -eq "failed" -and -not [bool]$providerHardAnalysis.pass -and $providerHardAnalysis.failure_type -eq "deterministic_hard_failure" -and $providerHardAnalysis.deterministic_hard_failure_count -eq 1 -and $providerHardAnalysis.severity_counts.total -eq 0 -and $providerHardAnalysis.deterministic_hard_failures[0].finding.code -eq "critical_text_clipped") "provider deterministic hard failure independently fails and is retained"
        Assert-SelfTest ($providerHardManifest.status -eq "failed" -and $providerHardManifest.analysis.deterministic_hard_failure_count -eq 1 -and @($providerHardManifest.analysis.deterministic_hard_failures).Count -eq 1) "provider manifest preserves and gates deterministic hard failures"

        $providerForgedHardFailurePath = Join-FullPath $analysisFixtureRoot "provider-forged-hard-failure.json"
        $providerForgedHardFailure = Read-JsonFile $providerHardFailurePath
        $providerForgedHardFailure.deterministic_hard_failures[0].capture_id = "forged.capture.initial"
        $providerForgedHardFailure | ConvertTo-Json -Depth 20 | Set-Content -Path $providerForgedHardFailurePath -Encoding UTF8
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest-provider-forged-hard-failure" -Results @($passed) -ScreensValue @("ui_gallery") -DevicesValue @($devices[0]) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($devices[0]) -AnalysisModeName "Provider" -AnalysisResultFile $providerForgedHardFailurePath
        $providerForgedHardAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($providerForgedHardAnalysis.status -eq "failed" -and $providerForgedHardAnalysis.failure_type -eq "ai_result_invalid") "provider forged hard failure capture is rejected"

        $rawProviderResultPath = Join-FullPath $analysisFixtureRoot "raw-provider.json"
        [ordered]@{ schema_version = 1; issues = @() } | ConvertTo-Json -Depth 5 | Set-Content -Path $rawProviderResultPath -Encoding UTF8
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest-provider-raw-rejected" -Results @($passed) -ScreensValue @("ui_gallery") -DevicesValue @($devices[0]) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($devices[0]) -AnalysisModeName "Provider" -AnalysisResultFile $rawProviderResultPath
        $rawProviderAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($rawProviderAnalysis.status -eq "failed" -and $rawProviderAnalysis.failure_type -eq "ai_result_invalid") "provider raw output is rejected"

        $missingPreservationPath = Join-FullPath $analysisFixtureRoot "provider-missing-preservation.json"
        $missingPreservation = Read-JsonFile $providerResultPath
        $missingPreservation.PSObject.Properties.Remove("deterministic_hard_failures_preserved")
        $missingPreservation | ConvertTo-Json -Depth 20 | Set-Content -Path $missingPreservationPath -Encoding UTF8
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest-provider-preservation-rejected" -Results @($passed) -ScreensValue @("ui_gallery") -DevicesValue @($devices[0]) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($devices[0]) -AnalysisModeName "Provider" -AnalysisResultFile $missingPreservationPath
        $missingPreservationAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($missingPreservationAnalysis.status -eq "failed" -and $missingPreservationAnalysis.failure_type -eq "ai_result_invalid") "provider missing preservation evidence is rejected"

        $blockingResultPath = Join-FullPath $analysisFixtureRoot "blocking.json"
        Write-FakeAnalysisResult -Path $blockingResultPath -Issues @(
            (New-FakeAnalysisIssue -Capture $passed.captures[0] -Severity "minor" -ProblemType "text_overlap" -Problem "文字重叠导致主按钮不可读")
        )
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest-blocking" -Results @($passed) -ScreensValue @("ui_gallery") -DevicesValue @($devices[0]) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($devices[0]) -AnalysisModeName "Fixture" -AnalysisResultFile $blockingResultPath
        $blockingAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($blockingAnalysis.status -eq "failed" -and $blockingAnalysis.failure_type -eq "ai_blocking_issue" -and $blockingAnalysis.severity_counts.blocking -eq 1) "blocking analysis fails gate"
        $blockingManifest = Read-JsonFile (Join-FullPath $tempRoot "manifest.json")
        Assert-SelfTest ($blockingManifest.status -eq "failed") "blocking analysis updates manifest status"

        $mediumResultPath = Join-FullPath $analysisFixtureRoot "medium.json"
        Write-FakeAnalysisResult -Path $mediumResultPath -Issues @(
            (New-FakeAnalysisIssue -Capture $passed.captures[0] -Severity "minor" -ProblemType "small_touch_target" -Problem "触控目标明显过小")
        )
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest-medium" -Results @($passed) -ScreensValue @("ui_gallery") -DevicesValue @($devices[0]) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($devices[0]) -AnalysisModeName "Fixture" -AnalysisResultFile $mediumResultPath
        $mediumAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($mediumAnalysis.status -eq "failed" -and $mediumAnalysis.issues[0].severity -eq "medium") "medium problem type blocks"

        $invalidJsonPath = Join-FullPath $analysisFixtureRoot "invalid.json"
        Set-Content -Path $invalidJsonPath -Value "{ invalid json" -Encoding UTF8
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest-invalid-json" -Results @($passed) -ScreensValue @("ui_gallery") -DevicesValue @($devices[0]) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($devices[0]) -AnalysisModeName "Fixture" -AnalysisResultFile $invalidJsonPath
        $invalidJsonAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($invalidJsonAnalysis.status -eq "failed" -and $invalidJsonAnalysis.failure_type -eq "ai_result_invalid") "invalid JSON analysis classification"

        $missingFieldPath = Join-FullPath $analysisFixtureRoot "missing-field.json"
        Write-FakeAnalysisResult -Path $missingFieldPath -Issues @(
            [ordered]@{
                screen = [string]$passed.captures[0].screen
                device = [string]$passed.captures[0].device
                state = [string]$passed.captures[0].state
                problem = "missing likely cause"
                evidence = "fixture evidence"
                suggested_files = @("project/src/game/screens/dev/ui_gallery.rs")
            }
        )
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest-missing-field" -Results @($passed) -ScreensValue @("ui_gallery") -DevicesValue @($devices[0]) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($devices[0]) -AnalysisModeName "Fixture" -AnalysisResultFile $missingFieldPath
        $missingFieldAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($missingFieldAnalysis.status -eq "failed" -and $missingFieldAnalysis.failure_type -eq "ai_result_invalid") "missing required field analysis classification"

        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest-missing-capture" -Results @($missing) -ScreensValue @("ui_gallery") -DevicesValue @($devices[1]) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($devices[1]) -AnalysisModeName "Fixture" -AnalysisResultFile $minorResultPath
        $missingCaptureAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($missingCaptureAnalysis.status -eq "failed" -and $missingCaptureAnalysis.failure_type -eq "ai_missing_capture_metadata") "missing screenshot metadata analysis classification"

        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest-missing-result" -Results @($passed) -ScreensValue @("ui_gallery") -DevicesValue @($devices[0]) -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @($devices[0]) -AnalysisModeName "Fixture" -AnalysisResultFile (Join-FullPath $analysisFixtureRoot "missing-result.json")
        $missingResultAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($missingResultAnalysis.status -eq "failed" -and $missingResultAnalysis.failure_type -eq "ai_analysis_failed") "missing analysis result classification"

        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "selftest" -Results $results -ScreensValue $screens -DevicesValue $devices -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue $devices
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
        $remoteAnalysisInput = Read-JsonFile (Join-FullPath $tempRoot "analysis-input.json")
        Assert-SelfTest (($remoteAnalysisInput.runner_mode -eq "remote") -and -not [string]::IsNullOrWhiteSpace([string]$remoteAnalysisInput.captures[0].screenshot_artifact_uri) -and @($remoteAnalysisInput.captures[0].remote_task_ids).Count -gt 0) "remote analysis input artifact task mapping"

        $remoteMinorResultPath = Join-FullPath $analysisFixtureRoot "remote-minor.json"
        Write-FakeAnalysisResult -Path $remoteMinorResultPath -Issues @(
            (New-FakeAnalysisIssue -Capture $remoteResult.captures[0] -Severity "minor" -ProblemType "visual_polish" -Problem "remote minor polish")
        )
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "remote-minor" -Results @($remoteResult) -ScreensValue @("ui_gallery") -DevicesValue @($remoteTargets[0].label) -IsDryRun $false -RerunSource "" -RunnerMode "Remote" -RemoteTargetsValue $remoteTargets -RemoteBackendName "Mock" -LocalDevicesValue @("desktop") -AnalysisModeName "Fixture" -AnalysisResultFile $remoteMinorResultPath
        $remoteMinorAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($remoteMinorAnalysis.status -eq "passed" -and $remoteMinorAnalysis.severity_counts.minor -eq 1) "remote minor analysis passes"

        $remoteBlockingResultPath = Join-FullPath $analysisFixtureRoot "remote-blocking.json"
        Write-FakeAnalysisResult -Path $remoteBlockingResultPath -Issues @(
            (New-FakeAnalysisIssue -Capture $remoteResult.captures[0] -Severity "medium" -ProblemType "critical_content_unreachable" -Problem "关键内容不可达")
        )
        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "remote-blocking" -Results @($remoteResult) -ScreensValue @("ui_gallery") -DevicesValue @($remoteTargets[0].label) -IsDryRun $false -RerunSource "" -RunnerMode "Remote" -RemoteTargetsValue $remoteTargets -RemoteBackendName "Mock" -LocalDevicesValue @("desktop") -AnalysisModeName "Fixture" -AnalysisResultFile $remoteBlockingResultPath
        $remoteBlockingAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($remoteBlockingAnalysis.status -eq "failed" -and $remoteBlockingAnalysis.failure_type -eq "ai_blocking_issue") "remote blocking analysis fails"

        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "remote-missing-artifact" -Results @($remoteMissingMetadataFailure) -ScreensValue @("ui_gallery") -DevicesValue @($remoteMissingMetadataTarget.label) -IsDryRun $false -RerunSource "" -RunnerMode "Remote" -RemoteTargetsValue @($remoteMissingMetadataTarget) -RemoteBackendName "Mock" -LocalDevicesValue @("desktop") -AnalysisModeName "Fixture" -AnalysisResultFile $remoteMinorResultPath
        $remoteMissingArtifactAnalysis = Read-JsonFile (Join-FullPath $tempRoot "analysis.json")
        Assert-SelfTest ($remoteMissingArtifactAnalysis.status -eq "failed" -and $remoteMissingArtifactAnalysis.failure_type -eq "ai_remote_artifact_read_failed") "remote missing artifact analysis classification"

        $policy = New-UiAuditFixPolicy
        $allowedPath = Test-UiAuditFixPathAllowed -RepoRoot $repoRoot -PathValue "project/src/game/screens/dev/ui_gallery.rs" -Policy $policy
        Assert-SelfTest ([bool]$allowedPath.allowed) "fix safety allows screen-local UI path"
        $forbiddenSummary = Test-UiAuditFixPathAllowed -RepoRoot $repoRoot -PathValue "summary/ui-audit/bad.rs" -Policy $policy
        Assert-SelfTest (-not [bool]$forbiddenSummary.allowed -and $forbiddenSummary.reason -like "forbidden_root:*") "fix safety rejects audit artifact path"
        $forbiddenTarget = Test-UiAuditFixPathAllowed -RepoRoot $repoRoot -PathValue "project/target/debug/build-output.rs" -Policy $policy
        Assert-SelfTest (-not [bool]$forbiddenTarget.allowed) "fix safety rejects build output path"
        $forbiddenEnv = Test-UiAuditFixPathAllowed -RepoRoot $repoRoot -PathValue ".env" -Policy $policy
        Assert-SelfTest (-not [bool]$forbiddenEnv.allowed) "fix safety rejects env files"

        $fixBase = Join-FullPath $tempRoot "fix-loop"
        New-Item -ItemType Directory -Force -Path $fixBase | Out-Null

        $script:LastUiAuditAnalysisStatus = $null
        $fixDefaultRoot = Join-FullPath $fixBase "default-off"
        $fixDefaultPassed = New-FakePassedUiAuditResult -RunRoot $fixDefaultRoot
        Write-FakeAnalysisResult -Path (Join-FullPath $analysisFixtureRoot "fix-default-blocking.json") -Issues @(
            (New-FakeAnalysisIssue -Capture $fixDefaultPassed.captures[0] -Severity "severe" -ProblemType "text_overlap" -Problem "blocking fixture for default off")
        )
        Write-UiAuditRunnerOutputs -RunRoot $fixDefaultRoot -RunIdValue "fix-default-off" -Results @($fixDefaultPassed) -ScreensValue @("ui_gallery") -DevicesValue @("phone-small") -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @("phone-small") -AnalysisModeName "Fixture" -AnalysisResultFile (Join-FullPath $analysisFixtureRoot "fix-default-blocking.json")
        $savedFixMode = $FixMode
        $savedMockFixScenario = $MockFixScenario
        $savedMaxFixIterations = $MaxFixIterations
        $FixMode = "Off"
        $exitDefaultOff = Resolve-UiAuditRunnerExitCode -Results @($fixDefaultPassed) -RunRoot $fixDefaultRoot -RunIdValue "fix-default-off" -RepoRoot $repoRoot -ProjectRoot $projectRoot
        $FixMode = $savedFixMode
        Assert-SelfTest ($exitDefaultOff -eq 1 -and -not (Test-Path (Join-FullPath $fixDefaultRoot "iterations"))) "fix loop default off does not start"

        $script:LastUiAuditAnalysisStatus = $null
        $fixMinorRoot = Join-FullPath $fixBase "minor-no-start"
        $fixMinorPassed = New-FakePassedUiAuditResult -RunRoot $fixMinorRoot
        $fixMinorPath = Join-FullPath $analysisFixtureRoot "fix-minor.json"
        Write-FakeAnalysisResult -Path $fixMinorPath -Issues @(
            (New-FakeAnalysisIssue -Capture $fixMinorPassed.captures[0] -Severity "minor" -ProblemType "visual_polish" -Problem "minor fixture")
        )
        Write-UiAuditRunnerOutputs -RunRoot $fixMinorRoot -RunIdValue "fix-minor" -Results @($fixMinorPassed) -ScreensValue @("ui_gallery") -DevicesValue @("phone-small") -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @("phone-small") -AnalysisModeName "Fixture" -AnalysisResultFile $fixMinorPath
        $FixMode = "Mock"
        $MockFixScenario = "Pass"
        $exitMinor = Resolve-UiAuditRunnerExitCode -Results @($fixMinorPassed) -RunRoot $fixMinorRoot -RunIdValue "fix-minor" -RepoRoot $repoRoot -ProjectRoot $projectRoot
        $minorManifest = Read-JsonFile (Join-FullPath $fixMinorRoot "manifest.json")
        Assert-SelfTest ($exitMinor -eq 0 -and $minorManifest.fix_loop.status -eq "skipped") "minor analysis does not start fix loop"

        $script:LastUiAuditAnalysisStatus = $null
        $fixPassRoot = Join-FullPath $fixBase "mock-pass"
        $fixPassResult = New-FakePassedUiAuditResult -RunRoot $fixPassRoot
        $fixBlockingPath = Join-FullPath $analysisFixtureRoot "fix-blocking.json"
        Write-FakeAnalysisResult -Path $fixBlockingPath -Issues @(
            (New-FakeAnalysisIssue -Capture $fixPassResult.captures[0] -Severity "severe" -ProblemType "text_overlap" -Problem "文字重叠导致主按钮不可读")
        )
        Write-UiAuditRunnerOutputs -RunRoot $fixPassRoot -RunIdValue "fix-mock-pass" -Results @($fixPassResult) -ScreensValue @("ui_gallery") -DevicesValue @("phone-small") -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @("phone-small") -AnalysisModeName "Fixture" -AnalysisResultFile $fixBlockingPath
        $FixMode = "Mock"
        $MockFixScenario = "Pass"
        $MaxFixIterations = 5
        $exitFixPass = Resolve-UiAuditRunnerExitCode -Results @($fixPassResult) -RunRoot $fixPassRoot -RunIdValue "fix-mock-pass" -RepoRoot $repoRoot -ProjectRoot $projectRoot
        $fixPassManifest = Read-JsonFile (Join-FullPath $fixPassRoot "manifest.json")
        Assert-SelfTest ($exitFixPass -eq 0 -and $fixPassManifest.fix_loop.status -eq "passed" -and $fixPassManifest.status -eq "passed") "mock fix loop clears blocking issue"
        Assert-SelfTest ((Test-Path (Join-FullPath $fixPassRoot "iterations/00-before/snapshot.json")) -and (Test-Path (Join-FullPath $fixPassRoot "iterations/01-after-fix/snapshot.json"))) "fix loop writes before and after snapshots"
        Assert-SelfTest (@($fixPassManifest.fix_loop.iterations[0].rerun_plan.devices).Count -eq 6) "local fix rerun plan uses full device matrix"
        Assert-SelfTest ((Test-Path (Join-FullPath $fixPassRoot "iterations/01-after-fix/checks/cargo-fmt.stdout.log")) -and (Test-Path (Join-FullPath $fixPassRoot "iterations/01-after-fix/checks/cargo-check.stdout.log"))) "fix loop preserves check logs"
        $fixPassReport = Get-Content -Raw -Path (Join-FullPath $fixPassRoot "report.md")
        Assert-SelfTest ($fixPassReport.Contains("## Fix Loop") -and $fixPassReport.Contains("After report")) "fix loop report section is written"
        Assert-SelfTest (@($fixPassManifest.artifact_links.fix_iterations).Count -eq 1 -and $fixPassManifest.artifact_links.fix_iterations[0].manifest.sha256 -match "^[0-9a-f]{64}$") "root manifest binds fix iteration artifacts"
        $linkedFixManifest = Read-JsonFile (Join-FullPath $fixPassRoot "iterations/01-after-fix/manifest.json")
        Assert-SelfTest ($linkedFixManifest.artifact_backlink.root_run_id -eq "fix-mock-pass" -and @($linkedFixManifest.artifact_backlink.capture_ids).Count -gt 0) "fix iteration manifest links back to root run and captures"
        Assert-SelfTest ($fixPassReport.Contains("## Artifact Links")) "report displays linked analysis and fix artifacts"

        $script:LastUiAuditAnalysisStatus = $null
        $fixMaxRoot = Join-FullPath $fixBase "mock-max"
        $fixMaxResult = New-FakePassedUiAuditResult -RunRoot $fixMaxRoot
        $fixMaxPath = Join-FullPath $analysisFixtureRoot "fix-max-blocking.json"
        Write-FakeAnalysisResult -Path $fixMaxPath -Issues @(
            (New-FakeAnalysisIssue -Capture $fixMaxResult.captures[0] -Severity "severe" -ProblemType "text_overlap" -Problem "blocking persists")
        )
        Write-UiAuditRunnerOutputs -RunRoot $fixMaxRoot -RunIdValue "fix-mock-max" -Results @($fixMaxResult) -ScreensValue @("ui_gallery") -DevicesValue @("phone-small") -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @("phone-small") -AnalysisModeName "Fixture" -AnalysisResultFile $fixMaxPath
        $FixMode = "Mock"
        $MockFixScenario = "MaxIterations"
        $MaxFixIterations = 2
        $exitFixMax = Resolve-UiAuditRunnerExitCode -Results @($fixMaxResult) -RunRoot $fixMaxRoot -RunIdValue "fix-mock-max" -RepoRoot $repoRoot -ProjectRoot $projectRoot
        $fixMaxManifest = Read-JsonFile (Join-FullPath $fixMaxRoot "manifest.json")
        Assert-SelfTest ($exitFixMax -eq 1 -and $fixMaxManifest.fix_loop.failure_type -eq "max_iterations_reached" -and @($fixMaxManifest.fix_loop.iterations).Count -eq 2 -and @($fixMaxManifest.fix_loop.final_issues).Count -gt 0) "mock fix loop reports max iterations"

        $script:LastUiAuditAnalysisStatus = $null
        $fixCheckRoot = Join-FullPath $fixBase "mock-check-failed"
        $fixCheckResult = New-FakePassedUiAuditResult -RunRoot $fixCheckRoot
        $fixCheckPath = Join-FullPath $analysisFixtureRoot "fix-check-blocking.json"
        Write-FakeAnalysisResult -Path $fixCheckPath -Issues @(
            (New-FakeAnalysisIssue -Capture $fixCheckResult.captures[0] -Severity "severe" -ProblemType "text_overlap" -Problem "blocking before check failure")
        )
        Write-UiAuditRunnerOutputs -RunRoot $fixCheckRoot -RunIdValue "fix-check-failed" -Results @($fixCheckResult) -ScreensValue @("ui_gallery") -DevicesValue @("phone-small") -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @("phone-small") -AnalysisModeName "Fixture" -AnalysisResultFile $fixCheckPath
        $FixMode = "Mock"
        $MockFixScenario = "CheckFailed"
        $MaxFixIterations = 5
        $exitCheck = Resolve-UiAuditRunnerExitCode -Results @($fixCheckResult) -RunRoot $fixCheckRoot -RunIdValue "fix-check-failed" -RepoRoot $repoRoot -ProjectRoot $projectRoot
        $fixCheckManifest = Read-JsonFile (Join-FullPath $fixCheckRoot "manifest.json")
        Assert-SelfTest ($exitCheck -eq 1 -and $fixCheckManifest.fix_loop.failure_type -eq "fix_check_failed" -and (Test-Path (Join-FullPath $fixCheckRoot "iterations/01-after-fix/checks/cargo-check.stderr.log"))) "mock fix loop reports check failure and logs"

        $script:LastUiAuditAnalysisStatus = $null
        $fixUnsafeRoot = Join-FullPath $fixBase "mock-unsafe"
        $fixUnsafeResult = New-FakePassedUiAuditResult -RunRoot $fixUnsafeRoot
        $fixUnsafePath = Join-FullPath $analysisFixtureRoot "fix-unsafe-blocking.json"
        Write-FakeAnalysisResult -Path $fixUnsafePath -Issues @(
            (New-FakeAnalysisIssue -Capture $fixUnsafeResult.captures[0] -Severity "severe" -ProblemType "text_overlap" -Problem "blocking before unsafe path")
        )
        Write-UiAuditRunnerOutputs -RunRoot $fixUnsafeRoot -RunIdValue "fix-unsafe" -Results @($fixUnsafeResult) -ScreensValue @("ui_gallery") -DevicesValue @("phone-small") -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @("phone-small") -AnalysisModeName "Fixture" -AnalysisResultFile $fixUnsafePath
        $FixMode = "Mock"
        $MockFixScenario = "UnsafePath"
        $exitUnsafe = Resolve-UiAuditRunnerExitCode -Results @($fixUnsafeResult) -RunRoot $fixUnsafeRoot -RunIdValue "fix-unsafe" -RepoRoot $repoRoot -ProjectRoot $projectRoot
        $fixUnsafeManifest = Read-JsonFile (Join-FullPath $fixUnsafeRoot "manifest.json")
        Assert-SelfTest ($exitUnsafe -eq 1 -and $fixUnsafeManifest.fix_loop.failure_type -eq "safety_policy_rejected" -and @($fixUnsafeManifest.fix_loop.safety_result.violations).Count -gt 0) "mock fix loop rejects unsafe changed path"

        $commandRepoRoot = Join-FullPath $fixBase "command-temp-repo"
        New-Item -ItemType Directory -Force -Path $commandRepoRoot | Out-Null
        $script:LastUiAuditAnalysisStatus = $null
        $fixCommandRoot = Join-FullPath $commandRepoRoot "run"
        $fixCommandResult = New-FakePassedUiAuditResult -RunRoot $fixCommandRoot
        $fixCommandPath = Join-FullPath $analysisFixtureRoot "fix-command-blocking.json"
        Write-FakeAnalysisResult -Path $fixCommandPath -Issues @(
            (New-FakeAnalysisIssue -Capture $fixCommandResult.captures[0] -Severity "severe" -ProblemType "text_overlap" -Problem "blocking before command unsafe path")
        )
        Write-UiAuditRunnerOutputs -RunRoot $fixCommandRoot -RunIdValue "fix-command-ignored-summary" -Results @($fixCommandResult) -ScreensValue @("ui_gallery") -DevicesValue @("phone-small") -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @("phone-small") -AnalysisModeName "Fixture" -AnalysisResultFile $fixCommandPath
        $commandUnsafe = 'New-Item -ItemType Directory -Force summary/ui-audit/unsafe-command | Out-Null; Set-Content summary/ui-audit/unsafe-command/bad.txt unsafe'
        $commandFixResult = Invoke-UiAuditFixLoop -RunRoot $fixCommandRoot -RunIdValue "fix-command-ignored-summary" -RepoRoot $commandRepoRoot -ProjectRoot $projectRoot -Mode "Command" -MaxIterations 5 -Command $commandUnsafe -MockScenario "Pass"
        $fixCommandManifest = Read-JsonFile (Join-FullPath $fixCommandRoot "manifest.json")
        Assert-SelfTest ($commandFixResult.exit_code -eq 1 -and $fixCommandManifest.fix_loop.failure_type -eq "safety_policy_rejected") "command fix loop rejects ignored summary write"
        Assert-SelfTest (@($fixCommandManifest.fix_loop.iterations[0].fixer.policy_changed_paths) -contains "summary/ui-audit/unsafe-command/bad.txt") "command fix loop records ignored changed path"
        Assert-SelfTest (@($fixCommandManifest.fix_loop.iterations[0].safety.violations | Where-Object { $_.relative -eq "summary/ui-audit/unsafe-command/bad.txt" -and $_.reason -like "forbidden_root:*" }).Count -eq 1) "command fix loop records violation reason for ignored summary write"

        $script:LastUiAuditAnalysisStatus = $null
        $fixCommandDeleteRoot = Join-FullPath $commandRepoRoot "delete-run"
        $preexistingDeletePath = Join-FullPath $commandRepoRoot "summary/ui-audit/preexisting-delete-test/old.txt"
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $preexistingDeletePath) | Out-Null
        Set-Content -Path $preexistingDeletePath -Value "old" -Encoding UTF8
        $fixCommandDeleteResult = New-FakePassedUiAuditResult -RunRoot $fixCommandDeleteRoot
        $fixCommandDeletePath = Join-FullPath $analysisFixtureRoot "fix-command-delete-blocking.json"
        Write-FakeAnalysisResult -Path $fixCommandDeletePath -Issues @(
            (New-FakeAnalysisIssue -Capture $fixCommandDeleteResult.captures[0] -Severity "severe" -ProblemType "text_overlap" -Problem "blocking before command delete unsafe path")
        )
        Write-UiAuditRunnerOutputs -RunRoot $fixCommandDeleteRoot -RunIdValue "fix-command-delete-ignored-summary" -Results @($fixCommandDeleteResult) -ScreensValue @("ui_gallery") -DevicesValue @("phone-small") -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @("phone-small") -AnalysisModeName "Fixture" -AnalysisResultFile $fixCommandDeletePath
        $commandDeleteUnsafe = 'Remove-Item -Force summary/ui-audit/preexisting-delete-test/old.txt'
        $commandDeleteFixResult = Invoke-UiAuditFixLoop -RunRoot $fixCommandDeleteRoot -RunIdValue "fix-command-delete-ignored-summary" -RepoRoot $commandRepoRoot -ProjectRoot $projectRoot -Mode "Command" -MaxIterations 5 -Command $commandDeleteUnsafe -MockScenario "Pass"
        $fixCommandDeleteManifest = Read-JsonFile (Join-FullPath $fixCommandDeleteRoot "manifest.json")
        Assert-SelfTest ($commandDeleteFixResult.exit_code -eq 1 -and $fixCommandDeleteManifest.fix_loop.failure_type -eq "safety_policy_rejected") "command fix loop rejects ignored summary delete"
        Assert-SelfTest (@($fixCommandDeleteManifest.fix_loop.iterations[0].fixer.policy_changed_paths) -contains "summary/ui-audit/preexisting-delete-test/old.txt") "command fix loop records ignored deleted path"
        Assert-SelfTest (@($fixCommandDeleteManifest.fix_loop.iterations[0].safety.violations | Where-Object { $_.relative -eq "summary/ui-audit/preexisting-delete-test/old.txt" -and $_.reason -like "forbidden_root:*" }).Count -eq 1) "command fix loop records violation reason for ignored summary delete"

        $boundaryPolicy = New-UiAuditFixPolicy
        $baselineBoundary = Test-UiAuditFixCommandBoundary -Command "cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- apply-baseline-update --plan plan.json" -Policy $boundaryPolicy
        $ordinaryBoundary = Test-UiAuditFixCommandBoundary -Command "cargo fmt --manifest-path project/Cargo.toml" -Policy $boundaryPolicy
        Assert-SelfTest (-not [bool]$baselineBoundary.allowed -and $baselineBoundary.failure_type -eq "baseline_update_forbidden") "fix loop rejects baseline update commands before execution"
        Assert-SelfTest ([bool]$ordinaryBoundary.allowed) "fix loop command boundary preserves ordinary UI fix commands"

        $baselineCommandRoot = Join-FullPath $commandRepoRoot "baseline-command-run"
        $baselineCommandResult = New-FakePassedUiAuditResult -RunRoot $baselineCommandRoot
        $baselineCommandAnalysis = Join-FullPath $analysisFixtureRoot "baseline-command-blocking.json"
        Write-FakeAnalysisResult -Path $baselineCommandAnalysis -Issues @(
            (New-FakeAnalysisIssue -Capture $baselineCommandResult.captures[0] -Severity "severe" -ProblemType "text_overlap" -Problem "blocking before forbidden baseline command")
        )
        Write-UiAuditRunnerOutputs -RunRoot $baselineCommandRoot -RunIdValue "baseline-command" -Results @($baselineCommandResult) -ScreensValue @("ui_gallery") -DevicesValue @("phone-small") -IsDryRun $false -RerunSource "" -RunnerMode "Local" -LocalDevicesValue @("phone-small") -AnalysisModeName "Fixture" -AnalysisResultFile $baselineCommandAnalysis
        $baselineMarker = Join-FullPath $commandRepoRoot "baseline-command-executed.txt"
        $baselineCommand = "Set-Content -Path '$baselineMarker' -Value bad; cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- apply-baseline-update --plan plan.json"
        $baselineCommandOutcome = Invoke-UiAuditFixLoop -RunRoot $baselineCommandRoot -RunIdValue "baseline-command" -RepoRoot $commandRepoRoot -ProjectRoot $projectRoot -Mode "Command" -MaxIterations 5 -Command $baselineCommand -MockScenario "Pass"
        $baselineCommandManifest = Read-JsonFile (Join-FullPath $baselineCommandRoot "manifest.json")
        Assert-SelfTest ($baselineCommandOutcome.failure_type -eq "baseline_update_forbidden" -and $baselineCommandManifest.fix_loop.failure_type -eq "baseline_update_forbidden") "fix loop records forbidden baseline command as a stable failure"
        Assert-SelfTest (-not (Test-Path -LiteralPath $baselineMarker)) "fix loop rejects baseline update command before any command side effect"

        Write-UiAuditRunnerOutputs -RunRoot $tempRoot -RunIdValue "remote-blocking-for-fix-plan" -Results @($remoteResult) -ScreensValue @("ui_gallery") -DevicesValue @($remoteTargets[0].label) -IsDryRun $false -RerunSource "" -RunnerMode "Remote" -RemoteTargetsValue $remoteTargets -RemoteBackendName "Mock" -LocalDevicesValue @("desktop") -AnalysisModeName "Fixture" -AnalysisResultFile $remoteBlockingResultPath
        $remoteManifestForFix = Read-JsonFile (Join-FullPath $tempRoot "manifest.json")
        $remoteFixPlan = New-UiAuditFixRerunPlan -Manifest $remoteManifestForFix -Issues @($remoteManifestForFix.analysis.issues)
        Assert-SelfTest ($remoteFixPlan.mode -eq "remote_related_target_matrix" -and @($remoteFixPlan.remote_targets).Count -eq 1) "remote fix rerun plan keeps related target matrix"

        $FixMode = $savedFixMode
        $MockFixScenario = $savedMockFixScenario
        $MaxFixIterations = $savedMaxFixIterations
        $script:LastUiAuditAnalysisStatus = $null

        Invoke-UiAuditStrictReferenceSelfTest -RepositoryRoot $repoRoot

        Write-Host "Self-test passed."
    } finally {
        if (Test-Path $tempRoot) {
            Remove-Item -Recurse -Force -Path $tempRoot
        }
    }
}

function Complete-UiAuditAndroidValidation {
    param(
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$Backend
    )

    if (-not $RequireRealAndroid) {
        return $null
    }
    $manifestPath = Join-FullPath $RunRoot "manifest.json"
    $manifest = Read-JsonFile $manifestPath
    $adb = Get-Command adb -ErrorAction SilentlyContinue
    $status = if ($Backend -ne "Http") {
        "external_unavailable"
    } elseif ($null -eq $adb) {
        "external_unavailable"
    } else {
        "pending_remote_metadata_validation"
    }
    $detail = if ($Backend -ne "Http") {
        "real Android validation requires -RemoteBackend Http; Mock artifacts are not device evidence"
    } elseif ($null -eq $adb) {
        "adb was not found; connect a real Android device and install the remote UI debug backend before validating status bar, safe area, fonts, and touch"
    } else {
        "adb is available, but the current remote protocol does not yet return Android system-bar/safe-area/font/touch metadata in a validated device contract"
    }
    $manifest | Add-Member -NotePropertyName "android_validation" -NotePropertyValue ([ordered]@{
            required = $true
            status = $status
            backend = $Backend
            adb_available = ($null -ne $adb)
            checks = @("status_bar", "safe_area", "font", "touch")
            detail = $detail
        }) -Force
    $manifest.status = "failed"
    $manifest | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
    Add-Content -LiteralPath (Join-FullPath $RunRoot "report.md") -Encoding UTF8 -Value ("`n## Android Device Validation`n`n- Status: ``$status`` `n- Detail: $detail`n")
    Write-Warning "Android device validation is ${status}: $detail"
    return $manifest.android_validation
}

function Invoke-UiAuditRunner {
    $effectiveMode = if ($Remote) { "Remote" } else { $Mode }
    if ($StrictReference -and [string]::IsNullOrWhiteSpace($ReferenceManifest)) {
        throw "-StrictReference requires -ReferenceManifest."
    }
    if (-not [string]::IsNullOrWhiteSpace($ReferenceManifest) -and -not $StrictReference) {
        throw "-ReferenceManifest requires explicit -StrictReference; ordinary audit mode remains reference-free."
    }
    if ($effectiveMode -eq "Remote" -and $DeterministicCapture) {
        throw "-DeterministicCapture currently supports local window profiles only; remote deterministic capture requires the stage 11 device contract."
    }
    if ($effectiveMode -eq "Remote" -and $StrictReference) {
        throw "Strict reference comparison requires a deterministic remote Android capture contract. The current remote runner records Mock/Http artifacts but does not yet provide that contract."
    }
    if ($RequireRealAndroid -and $effectiveMode -ne "Remote") {
        throw "-RequireRealAndroid is only meaningful with -Remote -RemoteBackend Http and a real Android capture backend."
    }

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

    $script:UiAuditClosedLoopGeneration = $null
    if ($GenerationMode -ne "Off") {
        if ($DryRun) {
            $script:UiAuditClosedLoopGeneration = [ordered]@{
                protocol_version = 1
                mode = $GenerationMode.ToLowerInvariant()
                status = "planned_by_audit_runner"
                detail = "dry run; closed-loop generation was not started"
            }
        } else {
            $script:UiAuditClosedLoopGeneration = Invoke-UiAuditClosedLoopGeneration -RepositoryRoot $repoRoot -Mode $GenerationMode
            Write-Host "Closed-loop generation: $($script:UiAuditClosedLoopGeneration.status)"
            if ($null -ne $script:UiAuditClosedLoopGeneration.audit_registration) {
                $registration = $script:UiAuditClosedLoopGeneration.audit_registration
                Write-Host "Draft runtime registration: $($registration.screen) / $($registration.device) / $($registration.states -join ', ')"
            }
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
            [void](Complete-UiAuditAndroidValidation -RunRoot $runRoot -Backend $RemoteBackend)
            Write-Host "Dry run complete. Remote adminapi tasks were not created."
            Write-Host "Manifest: $(Join-FullPath $runRoot "manifest.json")"
            Write-Host "Report: $(Join-FullPath $runRoot "report.md")"
            return Resolve-UiAuditRunnerExitCode -Results @($results.ToArray()) -RunRoot $runRoot -RunIdValue $runIdValue -RepoRoot $repoRoot -ProjectRoot $projectRoot
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
        [void](Complete-UiAuditAndroidValidation -RunRoot $runRoot -Backend $RemoteBackend)
        Write-Host "Manifest: $(Join-FullPath $runRoot "manifest.json")"
        Write-Host "Report: $(Join-FullPath $runRoot "report.md")"

        return Resolve-UiAuditRunnerExitCode -Results @($results.ToArray()) -RunRoot $runRoot -RunIdValue $runIdValue -RepoRoot $repoRoot -ProjectRoot $projectRoot
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
        if ($StrictReference) {
            $referenceEntries = @(Get-UiAuditReferenceEntries -RepositoryRoot $repoRoot -ManifestPath $ReferenceManifest -LocaleValue $Locale -ThemeValue $Theme)
            $seeds = @(Get-UiAuditReferenceTaskSeeds -References $referenceEntries)
            $screensToRun = @($seeds | ForEach-Object { [string]$_.screen } | Select-Object -Unique)
            $devicesToRun = @($seeds | ForEach-Object { [string]$_.device } | Select-Object -Unique)
            $tasks = @(New-UiAuditTasksFromSeeds -RunRoot $runRoot -Seeds $seeds -StateValue $States -ExtraBevyArgs $extraBevyArgs)
            Write-Host "Strict reference matrix: $($referenceEntries.Count) capture mappings across $($tasks.Count) runner tasks."
        } else {
            $screensToRun = @(Resolve-UiAuditScreens $Screens)
            $tasks = @(New-UiAuditTasks -RunRoot $runRoot -ScreensToRun $screensToRun -DevicesToRun $devicesToRun -StateValue $States -ExtraBevyArgs $extraBevyArgs)
        }
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
        [void](Complete-UiAuditReferenceComparison -RepositoryRoot $repoRoot -RunRoot $runRoot -RunIdValue $runIdValue -Results @($results.ToArray()) -IsDryRun $true)
        Write-Host "Dry run complete. No cargo process was started."
        Write-Host "Manifest: $(Join-FullPath $runRoot "manifest.json")"
        Write-Host "Report: $(Join-FullPath $runRoot "report.md")"
        return Resolve-UiAuditRunnerExitCode -Results @($results.ToArray()) -RunRoot $runRoot -RunIdValue $runIdValue -RepoRoot $repoRoot -ProjectRoot $projectRoot
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
    [void](Complete-UiAuditReferenceComparison -RepositoryRoot $repoRoot -RunRoot $runRoot -RunIdValue $runIdValue -Results @($results.ToArray()) -IsDryRun $false)
    Write-Host "Manifest: $(Join-FullPath $runRoot "manifest.json")"
    Write-Host "Report: $(Join-FullPath $runRoot "report.md")"

    return Resolve-UiAuditRunnerExitCode -Results @($results.ToArray()) -RunRoot $runRoot -RunIdValue $runIdValue -RepoRoot $repoRoot -ProjectRoot $projectRoot
}

if ($SelfTest) {
    Invoke-UiAuditSelfTest
    exit 0
}

$exitCode = Invoke-UiAuditRunner
exit $exitCode
