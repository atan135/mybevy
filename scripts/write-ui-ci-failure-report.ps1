[CmdletBinding(PositionalBinding = $false)]
param(
    [string]$OutputPath,
    [ValidateSet("offline_ci", "online_contract")]
    [string]$RunMode,
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"

function New-UiCiFailureReport {
    param(
        [Parameter(Mandatory = $true)][string]$Mode,
        [System.Collections.IDictionary]$UntrustedFailureEvidence = @{}
    )

    # This is intentionally a whitelist. Untrusted failure evidence is accepted only so the
    # deterministic self-test can prove it never becomes an artifact field or value.
    $null = $UntrustedFailureEvidence
    [ordered]@{
        schema_version = 1
        report_kind = "ui_ci_failure_boundary"
        run_mode = $Mode
        report_redacted = $true
        downloadable = $true
        excludes = @(
            "credentials",
            "accounts_and_pii",
            "reference_images_and_bytes",
            "reference_paths",
            "screenshots",
            "raw_model_output"
        )
        remediation = "Inspect the protected CI logs; this artifact intentionally contains no untrusted input."
    }
}

function Test-UiCiFailureReportBoundary {
    param(
        [Parameter(Mandatory = $true)][System.Collections.IDictionary]$Report,
        [Parameter(Mandatory = $true)][System.Collections.IDictionary]$ForbiddenEvidence
    )

    $json = $Report | ConvertTo-Json -Depth 4
    foreach ($field in $ForbiddenEvidence.Keys) {
        if ($Report.Contains($field)) {
            throw "Failure report contains prohibited field '$field'"
        }
        $value = [string]$ForbiddenEvidence[$field]
        if (-not [string]::IsNullOrEmpty($value) -and $json.Contains($value, [System.StringComparison]::Ordinal)) {
            throw "Failure report contains prohibited evidence from '$field'"
        }
    }
    if (-not $Report.report_redacted -or -not $Report.downloadable) {
        throw "Failure report did not retain the redacted download contract"
    }
    return $json
}

function Write-UiCiFailureReport {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Mode
    )

    $parent = Split-Path -Parent $Path
    if ([string]::IsNullOrWhiteSpace($parent)) {
        throw "Failure report requires a file below an explicit output directory"
    }
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    $report = New-UiCiFailureReport -Mode $Mode
    $json = Test-UiCiFailureReportBoundary -Report $report -ForbiddenEvidence @{}
    [System.IO.File]::WriteAllText($Path, $json + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
}

if ($SelfTest) {
    $untrustedEvidence = [ordered]@{
        api_token = "fixture-secret-not-a-real-key"
        account_email = "fixture.account@example.test"
        pii_phone = "+1-555-0100"
        reference_image_bytes = "fixture-reference-image-bytes"
        reference_path = "C:/private/reference.png"
        screenshot_bytes = "fixture-screenshot-bytes"
        screenshot_path = "C:/private/capture.png"
        raw_model_output = "fixture-raw-model-output"
    }
    $report = New-UiCiFailureReport -Mode "offline_ci" -UntrustedFailureEvidence $untrustedEvidence
    $null = Test-UiCiFailureReportBoundary -Report $report -ForbiddenEvidence $untrustedEvidence
    if ($report.excludes -notcontains "accounts_and_pii" -or
        $report.excludes -notcontains "reference_images_and_bytes" -or
        $report.excludes -notcontains "reference_paths" -or
        $report.excludes -notcontains "screenshots" -or
        $report.excludes -notcontains "raw_model_output") {
        throw "Failure report self-test did not preserve every exclusion category"
    }
    Write-Output "UI CI failure report self-test passed"
    exit 0
}

if ([string]::IsNullOrWhiteSpace($OutputPath) -or [string]::IsNullOrWhiteSpace($RunMode)) {
    throw "-OutputPath and -RunMode are required unless -SelfTest is used"
}

Write-UiCiFailureReport -Path $OutputPath -Mode $RunMode
