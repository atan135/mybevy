[CmdletBinding()]
param(
    [switch]$Locked,
    [switch]$Release = $true
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$projectRoot = Join-Path $repoRoot "project"
$androidTarget = Join-Path $projectRoot "target-android"
$jniLibs = Join-Path $repoRoot "android\app\src\main\jniLibs"
New-Item -ItemType Directory -Force -Path $androidTarget, $jniLibs | Out-Null

$args = @("ndk", "-t", "arm64-v8a", "-P", "26", "-o", $jniLibs, "rustc")
if ($Locked) { $args += "--locked" }
if ($Release) { $args += "--release" }
$args += @("--lib", "--crate-type", "cdylib")

Push-Location $projectRoot
$previousTargetDir = $env:CARGO_TARGET_DIR
$env:CARGO_TARGET_DIR = $androidTarget
try {
    & cargo @args
    exit $LASTEXITCODE
}
finally {
    if ($null -eq $previousTargetDir) {
        Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    } else {
        $env:CARGO_TARGET_DIR = $previousTargetDir
    }
    Pop-Location
}
