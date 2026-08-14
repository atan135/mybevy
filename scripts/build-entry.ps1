[CmdletBinding(PositionalBinding = $false)]
param(
    [ValidateSet("desktop", "headless", "fangyuan-bake", "ui-preview", "test", "release", "android")]
    [string]$Target = "desktop",
    [ValidateSet("check", "build", "run")]
    [string]$Action = "check",
    [ValidateSet("dev", "dev-fast", "perf", "release")]
    [string]$Profile = "dev",
    [switch]$Locked,
    [switch]$DesktopFast,
    [switch]$AndroidApk,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$PassthroughArgs = @()
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$projectRoot = Join-Path $repoRoot "project"

function Add-LockedArgument {
    param([string[]]$Arguments)
    if ($Locked) { return @($Arguments + "--locked") }
    return @($Arguments)
}

function Invoke-CargoTarget {
    param(
        [string[]]$Arguments,
        [string]$WorkingDirectory = $projectRoot
    )

    $started = [System.Diagnostics.Stopwatch]::StartNew()
    $command = "cargo " + ($Arguments -join " ")
    Write-Host "build-entry target=$Target action=$Action command=$command"
    try {
        & cargo @Arguments
        $exitCode = $LASTEXITCODE
    }
    finally {
        $started.Stop()
    }
    Write-Host ("build-entry target={0} status={1} elapsed_seconds={2}" -f $Target, $(if ($exitCode -eq 0) { "passed" } else { "failed" }), [math]::Round($started.Elapsed.TotalSeconds, 3))
    exit $exitCode
}

if ($Target -eq "test" -and $Action -ne "check") {
    throw "Target 'test' only supports -Action check; use the target's test command directly for custom filters."
}
if ($Target -eq "android" -and $Action -eq "run") {
    throw "Target 'android' does not support -Action run. Use -AndroidApk to append Gradle assembleDebug."
}
if ($Target -ne "desktop" -and $DesktopFast) {
    throw "-DesktopFast is only valid for the desktop target."
}
if ($AndroidApk -and $Target -ne "android") {
    throw "-AndroidApk is only valid for the android target."
}
if ($DesktopFast -and $Profile -notin @("dev", "dev-fast")) {
    throw "-DesktopFast requires the dev or dev-fast profile; release/perf builds must not enable dynamic linking."
}
if ($Target -eq "android" -and $Profile -ne "release") {
    throw "Android entry currently supports the release Rust profile only; pass -Profile release."
}

if ($Target -eq "android") {
    $androidScript = Join-Path $repoRoot "scripts\build-android-rust.ps1"
    $androidArguments = @{}
    if ($Locked) { $androidArguments.Locked = $true }
    $androidArguments.Release = $true
    $started = [System.Diagnostics.Stopwatch]::StartNew()
    Write-Host "build-entry target=android action=$Action command=build-android-rust.ps1"
    try {
        & $androidScript @androidArguments
        $exitCode = $LASTEXITCODE
        if ($exitCode -eq 0 -and $AndroidApk) {
            Push-Location (Join-Path $repoRoot "android")
            try {
                & .\gradlew.bat assembleDebug --no-daemon --stacktrace
                $exitCode = $LASTEXITCODE
            }
            finally {
                Pop-Location
            }
        }
    }
    finally {
        $started.Stop()
    }
    Write-Host ("build-entry target=android status={0} elapsed_seconds={1}" -f $(if ($exitCode -eq 0) { "passed" } else { "failed" }), [math]::Round($started.Elapsed.TotalSeconds, 3))
    exit $exitCode
}

$lockedArguments = if ($Locked) { @("--locked") } else { @() }
$binary = switch ($Target) {
    "desktop" { "project" }
    "headless" { "lockstep-sim-headless" }
    "fangyuan-bake" { "fangyuan_bake" }
    "ui-preview" { "ui-document-preview" }
    "release" { "project" }
}

if ($Target -eq "test") {
    Invoke-CargoTarget (Add-LockedArgument @("test", "--manifest-path", (Join-Path $projectRoot "Cargo.toml"), "--lib"))
}

$effectiveProfile = if ($Target -eq "release") { "release" } elseif ($DesktopFast) { "dev-fast" } else { $Profile }
$effectiveProfileArguments = if ($effectiveProfile -eq "dev") { @() } else { @("--profile", $effectiveProfile) }
$featureArguments = if ($Target -eq "ui-preview") { @("--features", "ui-document-preview-tool") } elseif ($DesktopFast) { @("--features", "bevy/dynamic_linking") } else { @() }
$command = if ($Action -eq "run") { "run" } elseif ($Action -eq "build" -or $Target -eq "release") { "build" } else { "check" }
$arguments = @($command, "--manifest-path", (Join-Path $projectRoot "Cargo.toml"), "--bin", $binary) + $effectiveProfileArguments + $featureArguments + $lockedArguments
if ($Action -eq "run" -or $Action -eq "build") {
    $arguments += @("--") + $PassthroughArgs
}
Invoke-CargoTarget $arguments
