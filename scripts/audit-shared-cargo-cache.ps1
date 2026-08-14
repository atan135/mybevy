[CmdletBinding(PositionalBinding = $false)]
param(
    [string]$OutputDirectory = "artifacts/shared-cache-audit",
    [switch]$IncludeFileSizes
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$artifactsRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "artifacts"))
$outputRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $OutputDirectory))

function Test-PathInside {
    param([Parameter(Mandatory = $true)][string]$Candidate, [Parameter(Mandatory = $true)][string]$Container)
    $containerPath = [System.IO.Path]::GetFullPath($Container).TrimEnd([char[]]@('\', '/'))
    $candidatePath = [System.IO.Path]::GetFullPath($Candidate).TrimEnd([char[]]@('\', '/'))
    return $candidatePath.Equals($containerPath, [System.StringComparison]::OrdinalIgnoreCase) -or
        $candidatePath.StartsWith($containerPath + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)
}

if (-not (Test-PathInside $outputRoot $artifactsRoot)) {
    throw "OutputDirectory must remain under ignored artifacts/: $outputRoot"
}
$runDirectory = Join-Path $outputRoot ([DateTimeOffset]::Now.ToString("yyyyMMdd-HHmmss"))
New-Item -ItemType Directory -Path $runDirectory -Force | Out-Null

function Get-Bytes {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Container)) { return [int64]0 }
    $sum = [int64]0
    foreach ($file in [System.IO.Directory]::EnumerateFiles($Path, "*", [System.IO.SearchOption]::AllDirectories)) {
        try { $sum += [int64]([System.IO.FileInfo]::new($file)).Length } catch { }
    }
    return $sum
}

function Get-TreeSnapshot {
    param([string]$Path)
    $exists = Test-Path -LiteralPath $Path -PathType Container
    $bytes = if ($exists -and $IncludeFileSizes) { Get-Bytes $Path } else { $null }
    return [pscustomobject]@{
        path = $Path.Substring($repoRoot.Length).TrimStart('\', '/')
        exists = $exists
        bytes = $bytes
        megabytes = if ($null -eq $bytes) { $null } else { [math]::Round($bytes / 1MB, 2) }
    }
}

$targetPaths = @(
    (Join-Path $repoRoot "target"),
    (Join-Path $repoRoot "project\target"),
    (Join-Path $repoRoot "project\target-android"),
    (Join-Path $repoRoot "tools\ui-generation\target"),
    (Join-Path $repoRoot "tools\ui-visual-audit\target"),
    (Join-Path $repoRoot "android\app\build"),
    (Join-Path $repoRoot "android\.gradle")
)
$processes = @(Get-Process cargo,rustc,rustdoc,link,project,java,gradle,adb -ErrorAction SilentlyContinue | Select-Object ProcessName,Id,CPU,StartTime,Path)
$lockEvidence = [System.Collections.Generic.List[object]]::new()
foreach ($path in $targetPaths | Where-Object { Test-Path -LiteralPath $_ -PathType Container }) {
    foreach ($file in (Get-ChildItem -LiteralPath $path -Recurse -File -ErrorAction SilentlyContinue)) {
        if ($file.Extension -notin @(".log", ".txt") -and $file.Name -notin @("stderr", "stdout")) { continue }
        foreach ($match in @(Select-String -LiteralPath $file.FullName -Pattern "(?i)(blocking )?waiting for file lock|could not acquire lock" -ErrorAction SilentlyContinue)) {
            $lockEvidence.Add([pscustomobject]@{ path = $match.Path.Substring($repoRoot.Length).TrimStart('\', '/'); line_number = $match.LineNumber; line = $match.Line })
        }
    }
}
$clearScript = Join-Path $repoRoot "scripts\clear-shared-cargo-target.ps1"
$previewOutput = @()
$previewExit = 0
try { $previewOutput = @(& $clearScript -IncrementalOnly 2>&1); $previewExit = $LASTEXITCODE } catch { $previewOutput = @($_.Exception.Message); $previewExit = 1 }
$metadata = [ordered]@{}
foreach ($manifest in @("project\Cargo.toml", "tools\ui-generation\Cargo.toml", "tools\ui-visual-audit\Cargo.toml")) {
    $manifestPath = Join-Path $repoRoot $manifest
    $metadata[$manifest.Replace('\', '/')] = if (Get-Command cargo -ErrorAction SilentlyContinue) { (& cargo metadata --no-deps --format-version 1 --manifest-path $manifestPath 2>$null | ConvertFrom-Json).target_directory } else { "cargo unavailable" }
}
$report = [ordered]@{
    schema = "mybevy.cargo-cache-audit.v2"
    timestamp = [DateTimeOffset]::Now.ToString("o")
    repository = $repoRoot
    target_policy = [ordered]@{
        legacy_root = "target (must be empty/removed after migration)"
        project_desktop = "project/target"
        project_android = "project/target-android"
        ui_generation = "tools/ui-generation/target"
        ui_visual_audit = "tools/ui-visual-audit/target"
        android_gradle = "android/.gradle and android/app/build"
    }
    target_paths = @($targetPaths | ForEach-Object { Get-TreeSnapshot $_ })
    processes = $processes
    lock_evidence = $lockEvidence
    clear_preview = [pscustomobject]@{ exit_code = $previewExit; output = $previewOutput; executed = $false }
    metadata_target_directories = $metadata
    notes = @("Root .cargo/config.toml target-dir override has been removed.", "No cleanup or cargo clean was executed by this audit.", "project/target-android is reserved for Android Rust builds and is never a cleanup target of the legacy script.")
}
$jsonPath = Join-Path $runDirectory "audit.json"
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $jsonPath -Encoding UTF8
$md = @(
    "# Cargo Cache Audit",
    "",
    "- Timestamp: $($report.timestamp)",
    "- Active build processes: $(@($processes).Count)",
    "- Lock evidence entries: $(@($lockEvidence).Count)",
    "- Legacy clear preview exit code: $previewExit (no files deleted)",
    "",
    "| Path | Exists | Size (MB) |",
    "| --- | --- | ---: |"
)
foreach ($item in $report.target_paths) { $md += "| $($item.path) | $($item.exists) | $($item.megabytes) |" }
$md += "", "Metadata target directories: ``audit.json``; no target cleanup was executed."
$md -join "`r`n" | Set-Content -LiteralPath (Join-Path $runDirectory "audit.md") -Encoding UTF8
Write-Host "Cargo cache audit: $(Join-Path $runDirectory 'audit.md')"
Write-Host "Raw audit: $jsonPath"
