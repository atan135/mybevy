[CmdletBinding(PositionalBinding = $false)]
param(
    [Parameter(Mandatory = $true)]
    [string]$ReferenceManifest,
    [string]$RunId = "ui-audit-ci",
    [int]$TimeoutSeconds = 900,
    [switch]$OnlineAi
)

$ErrorActionPreference = "Stop"

$scriptRoot = if (-not [string]::IsNullOrWhiteSpace($PSScriptRoot)) {
    $PSScriptRoot
} else {
    Split-Path -Parent $PSCommandPath
}
$runner = Join-Path $scriptRoot "run-ui-audit.ps1"
if (-not (Test-Path -LiteralPath $runner -PathType Leaf)) {
    throw "UI audit runner is missing: $runner"
}
if (-not (Test-Path -LiteralPath $ReferenceManifest -PathType Leaf)) {
    throw "Reference manifest is missing: $ReferenceManifest"
}

$comparisonAiMode = "Fixture"
if ($OnlineAi) {
    if ([string]::IsNullOrWhiteSpace([string]$env:MYBEVY_UI_AUDIT_AI_CONFIG) -or
        -not (Test-Path -LiteralPath $env:MYBEVY_UI_AUDIT_AI_CONFIG -PathType Leaf)) {
        throw "-OnlineAi requires MYBEVY_UI_AUDIT_AI_CONFIG to point to an explicit provider config."
    }
    $comparisonAiMode = "Provider"
}

Write-Host "UI audit CI mode: deterministic strict comparison with $comparisonAiMode AI"
& $runner `
    -StrictReference `
    -ReferenceManifest $ReferenceManifest `
    -DeterministicCapture `
    -ComparisonAiMode $comparisonAiMode `
    -AnalysisMode Off `
    -RunId $RunId `
    -TimeoutSeconds $TimeoutSeconds
exit $LASTEXITCODE
