[CmdletBinding(PositionalBinding = $false)]
param(
    [string[]]$ChangedFiles = @(),
    [string[]]$Labels = @(),
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"

$approvalLabel = "ui-reference-baseline-approved"
$protectedPrefixes = @(
    "tools/ui-visual-audit/fixtures/references/",
    "tools/ui-visual-audit/fixtures/baselines/",
    "tools/ui-visual-audit/fixtures/masks/",
    "tools/ui-visual-audit/fixtures/thresholds/"
)

function Test-UiReferenceBaselineApproval {
    param(
        [string[]]$Paths = @(),
        [string[]]$PullRequestLabels = @()
    )

    $protected = @(
        $Paths | Where-Object {
            $normalized = $_.Replace("\", "/")
            $protectedPrefixes | Where-Object { $normalized.StartsWith($_, [System.StringComparison]::Ordinal) }
        }
    )
    $approved = $PullRequestLabels -contains $approvalLabel
    if ($protected.Count -gt 0 -and -not $approved) {
        throw "Reference/baseline changes require the '$approvalLabel' PR label: $($protected -join ', ')"
    }

    [pscustomobject]@{
        approval_label = $approvalLabel
        protected_change_count = $protected.Count
        approved = $approved
        status = if ($protected.Count -eq 0 -or $approved) { "passed" } else { "blocked" }
    }
}

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Name)

    try {
        & $Action
    } catch {
        return
    }
    throw "Self-test expected rejection: $Name"
}

if ($SelfTest) {
    $ordinary = Test-UiReferenceBaselineApproval -Paths @("tools/ui-generation/src/main.rs") -PullRequestLabels @()
    if ($ordinary.status -ne "passed" -or $ordinary.protected_change_count -ne 0) {
        throw "Self-test ordinary source change should pass without approval"
    }
    Assert-Rejected {
        Test-UiReferenceBaselineApproval -Paths @("tools/ui-visual-audit/fixtures/references/phone.png") -PullRequestLabels @()
    } "reference change without approval label"
    Assert-Rejected {
        Test-UiReferenceBaselineApproval -Paths @("tools/ui-visual-audit/fixtures/baselines/revision.json") -PullRequestLabels @("unrelated")
    } "baseline change with unrelated label"
    $approved = Test-UiReferenceBaselineApproval -Paths @("tools/ui-visual-audit/fixtures/references/phone.png") -PullRequestLabels @($approvalLabel)
    if ($approved.status -ne "passed" -or -not $approved.approved) {
        throw "Self-test approved reference change should pass"
    }
    Write-Output "UI reference/baseline approval self-test passed"
    exit 0
}

Test-UiReferenceBaselineApproval -Paths $ChangedFiles -PullRequestLabels $Labels | ConvertTo-Json -Compress
