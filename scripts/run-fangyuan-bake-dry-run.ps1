param(
    [string]$InputDir = "project/assets/fangyuan",
    [string]$OutputDir = "artifacts/fangyuan-bake/dry-run/out",
    [string]$ReportPath = "artifacts/fangyuan-bake/dry-run/report.txt",
    [switch]$KeepReportDir
)

$ErrorActionPreference = "Stop"

function Resolve-RepoRoot {
    $scriptRoot = Split-Path -Parent $PSCommandPath
    return (Resolve-Path (Join-Path $scriptRoot "..")).Path
}

function Resolve-RepoPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return (Join-Path $RepoRoot $Path)
}

$repoRoot = Resolve-RepoRoot
$projectRoot = Join-Path $repoRoot "project"
$inputPath = Resolve-RepoPath -Path $InputDir -RepoRoot $repoRoot
$outputPath = Resolve-RepoPath -Path $OutputDir -RepoRoot $repoRoot
$reportPathResolved = Resolve-RepoPath -Path $ReportPath -RepoRoot $repoRoot
$reportDir = Split-Path -Parent $reportPathResolved

if (-not (Test-Path -LiteralPath $inputPath -PathType Container)) {
    throw "Fangyuan bake input directory not found: $inputPath"
}

if (-not $KeepReportDir -and (Test-Path -LiteralPath $reportPathResolved -PathType Leaf)) {
    Remove-Item -LiteralPath $reportPathResolved -Force
}

if ($reportDir) {
    New-Item -ItemType Directory -Path $reportDir -Force | Out-Null
}

$cargoArgs = @(
    "run",
    "--quiet",
    "--bin",
    "fangyuan_bake",
    "--",
    "--input",
    $inputPath,
    "--output",
    $outputPath,
    "--dry-run",
    "--report",
    $reportPathResolved
)

Push-Location $projectRoot
$previousCargoIncremental = $env:CARGO_INCREMENTAL
try {
    $env:CARGO_INCREMENTAL = "0"
    Write-Host "fangyuan bake dry-run: input=$inputPath"
    Write-Host "fangyuan bake dry-run: report=$reportPathResolved"
    & cargo @cargoArgs
    $exitCode = $LASTEXITCODE
}
finally {
    $env:CARGO_INCREMENTAL = $previousCargoIncremental
    Pop-Location
}

if ($exitCode -ne 0) {
    if (Test-Path -LiteralPath $reportPathResolved) {
        Write-Host "fangyuan bake dry-run report:"
        Get-Content -LiteralPath $reportPathResolved
    }
    exit $exitCode
}

if (-not (Test-Path -LiteralPath $reportPathResolved -PathType Leaf)) {
    throw "Fangyuan bake dry-run completed without writing report: $reportPathResolved"
}

Write-Host "fangyuan bake dry-run passed."
Get-Content -LiteralPath $reportPathResolved
