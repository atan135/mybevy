[CmdletBinding(PositionalBinding = $false)]
param(
    [string]$RepositoryRoot = (Split-Path -Parent $PSScriptRoot),
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"

function Test-CargoLockSupplyChain {
    param(
        [Parameter(Mandatory = $true)]
        [string]$LockContent,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $packages = $LockContent -split "(?m)^\[\[package\]\]\s*$" | Select-Object -Skip 1
    if ($packages.Count -eq 0) {
        throw "$Name has no package entries"
    }
    foreach ($package in $packages) {
        $source = [regex]::Match($package, '(?m)^source = "([^"]+)"\s*$')
        if (-not $source.Success) {
            continue
        }
        if ($source.Groups[1].Value -notlike "registry+https://github.com/rust-lang/crates.io-index") {
            throw "$Name contains a non-registry or unpinned dependency source"
        }
        $checksum = [regex]::Match($package, '(?m)^checksum = "([0-9a-f]{64})"\s*$')
        if (-not $checksum.Success) {
            throw "$Name registry dependency is missing a SHA-256 checksum"
        }
    }
}

function Test-UiSupplyChainPolicy {
    param([Parameter(Mandatory = $true)][string]$Root)

    $policyPath = Join-Path $Root "tools/ui-generation/fixtures/ci/ui-ci-security-policy.v1.json"
    $policy = Get-Content -Raw -LiteralPath $policyPath | ConvertFrom-Json
    if (-not $policy.supply_chain.require_locked_cargo -or
        -not $policy.supply_chain.reject_git_dependencies -or
        -not $policy.supply_chain.generated_resources_require_license -or
        -not $policy.supply_chain.model_output_requires_human_approval -or
        $policy.supply_chain.untrusted_shader_execution -ne "forbidden" -or
        -not $policy.supply_chain.approved_shader_license_required) {
        throw "UI supply-chain policy is not fail-closed"
    }
    if ($policy.artifacts.include_original_credentials -or $policy.artifacts.include_unapproved_reference_images) {
        throw "UI artifact policy permits unsafe data retention"
    }

    $catalogPath = Join-Path $Root "tools/ui-generation/assets/ui_asset_catalog.v1.json"
    $catalog = Get-Content -Raw -LiteralPath $catalogPath | ConvertFrom-Json
    if ($catalog.schema_version -ne 1 -or @($catalog.assets).Count -eq 0) {
        throw "UI asset catalog is missing its versioned license inventory"
    }
    foreach ($asset in @($catalog.assets)) {
        if ($null -eq $asset.license -or [string]::IsNullOrWhiteSpace([string]$asset.license.status)) {
            throw "Asset catalog entry '$($asset.asset_id)' has no license status"
        }
        if ($asset.license.status -ne "unknown" -and [string]::IsNullOrWhiteSpace([string]$asset.license.reference)) {
            throw "Licensed asset catalog entry '$($asset.asset_id)' has no license reference"
        }
    }

    $shaderExtensions = @("*.wgsl", "*.glsl", "*.vert", "*.frag", "*.spv", "*.shader")
    $shaderRoots = @(
        (Join-Path $Root "tools/ui-generation/fixtures"),
        (Join-Path $Root "summary/ui-generation")
    )
    foreach ($shaderRoot in $shaderRoots) {
        if (-not (Test-Path -LiteralPath $shaderRoot -PathType Container)) {
            continue
        }
        $unexpected = Get-ChildItem -LiteralPath $shaderRoot -File -Recurse -Include $shaderExtensions
        if (@($unexpected).Count -gt 0) {
            throw "Untrusted generated or fixture shader execution is forbidden: $($unexpected[0].FullName)"
        }
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
    Test-CargoLockSupplyChain -Name "valid fixture" -LockContent @'
[[package]]
name = "fixture"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
'@
    Assert-Rejected {
        Test-CargoLockSupplyChain -Name "git fixture" -LockContent @'
[[package]]
name = "fixture"
version = "1.0.0"
source = "git+https://example.test/repository"
'@
    } "git dependency source"
    Assert-Rejected {
        Test-CargoLockSupplyChain -Name "checksum fixture" -LockContent @'
[[package]]
name = "fixture"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
'@
    } "registry dependency without checksum"
    Write-Output "UI supply-chain self-test passed"
    exit 0
}

$root = (Resolve-Path -LiteralPath $RepositoryRoot -ErrorAction Stop).Path
Test-CargoLockSupplyChain -Name "project/Cargo.lock" -LockContent (Get-Content -Raw -LiteralPath (Join-Path $root "project/Cargo.lock"))
Test-CargoLockSupplyChain -Name "tools/ui-generation/Cargo.lock" -LockContent (Get-Content -Raw -LiteralPath (Join-Path $root "tools/ui-generation/Cargo.lock"))
Test-CargoLockSupplyChain -Name "tools/ui-visual-audit/Cargo.lock" -LockContent (Get-Content -Raw -LiteralPath (Join-Path $root "tools/ui-visual-audit/Cargo.lock"))
Test-UiSupplyChainPolicy -Root $root
[pscustomobject]@{
    status = "passed"
    checked_locks = @("project/Cargo.lock", "tools/ui-generation/Cargo.lock", "tools/ui-visual-audit/Cargo.lock")
    generated_resource_license_policy = "required"
    model_output_policy = "human_approval_required"
    untrusted_shader_execution = "forbidden"
} | ConvertTo-Json -Compress
