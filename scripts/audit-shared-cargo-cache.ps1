[CmdletBinding(PositionalBinding = $false)]
param(
    [string]$OutputDirectory = "artifacts/shared-cache-audit",
    [switch]$IncludeFileSizes
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$artifactsRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "artifacts"))
$outputRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $OutputDirectory))
$artifactsPrefix = $artifactsRoot.TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar
if (-not ($outputRoot.Equals($artifactsRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
    $outputRoot.StartsWith($artifactsPrefix, [System.StringComparison]::OrdinalIgnoreCase))) {
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
    [pscustomobject]@{
        path = $Path.Substring($repoRoot.Length).TrimStart('\', '/')
        exists = $exists
        bytes = $bytes
        megabytes = if ($null -eq $bytes) { $null } else { [math]::Round($bytes / 1MB, 2) }
        entries = if ($exists) { @(Get-ChildItem -LiteralPath $Path -Force | Select-Object Name,Mode,Length,LastWriteTime) } else { @() }
    }
}

$targetPath = Join-Path $repoRoot "target"
$incrementalPath = Join-Path $targetPath "debug\incremental"
$processes = @(Get-Process cargo,rustc,link,project,java,gradle,adb -ErrorAction SilentlyContinue |
    Select-Object ProcessName,Id,CPU,StartTime,Path)
$lockEvidence = [System.Collections.Generic.List[object]]::new()
foreach ($file in (Get-ChildItem -LiteralPath $targetPath -Recurse -File -ErrorAction SilentlyContinue)) {
    if ($file.Name -notin @("stderr", "stdout") -and $file.Extension -notin @(".log", ".txt")) { continue }
    foreach ($match in @(Select-String -LiteralPath $file.FullName -Pattern "(?i)(blocking )?waiting for file lock|could not acquire lock" -ErrorAction SilentlyContinue)) {
        $lockEvidence.Add([pscustomobject]@{ path = $match.Path.Substring($repoRoot.Length).TrimStart('\', '/'); line_number = $match.LineNumber; line = $match.Line })
    }
}

$clearScript = Join-Path $repoRoot "scripts\clear-shared-cargo-target.ps1"
$previewOutput = @()
$previewExit = 0
try {
    $previewOutput = @(& $clearScript -IncrementalOnly 2>&1)
    $previewExit = $LASTEXITCODE
} catch {
    $previewOutput = @($_.Exception.Message)
    $previewExit = 1
}
$config = Get-Content (Join-Path $repoRoot ".cargo\config.toml") -Raw
$manifests = [ordered]@{}
foreach ($manifest in @("project\Cargo.toml", "tools\ui-generation\Cargo.toml", "tools\ui-visual-audit\Cargo.toml")) {
    $manifests[$manifest.Replace('\', '/')] = [System.IO.File]::ReadAllText((Join-Path $repoRoot $manifest))
}
$report = [ordered]@{
    schema = "mybevy.shared-cargo-cache-audit.v1"
    timestamp = [DateTimeOffset]::Now.ToString("o")
    repository = $repoRoot
    shared_config = $config
    manifests = $manifests
    target_paths = @(
        (Get-TreeSnapshot $targetPath),
        (Get-TreeSnapshot (Join-Path $repoRoot "project\target")),
        (Get-TreeSnapshot (Join-Path $repoRoot "tools\ui-generation\target")),
        (Get-TreeSnapshot (Join-Path $repoRoot "tools\ui-visual-audit\target")),
        (Get-TreeSnapshot (Join-Path $repoRoot "android\app\build")),
        (Get-TreeSnapshot (Join-Path $repoRoot "android\.gradle"))
    )
    incremental = Get-TreeSnapshot $incrementalPath
    processes = $processes
    lock_evidence = $lockEvidence
    clear_preview = [pscustomobject]@{ exit_code = $previewExit; output = $previewOutput; executed = $false }
    notes = @(
        "Root .cargo/config.toml sets target-dir = target for all manifests.",
        "No cleanup or cargo clean was executed by this audit.",
        "project/target and tool-local target paths are recorded to detect accidental overrides."
    )
}
$jsonPath = Join-Path $runDirectory "audit.json"
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $jsonPath -Encoding UTF8
$md = @(
    "# Shared Cargo Cache Audit",
    "",
    "- Timestamp: $($report.timestamp)",
    "- Shared config: ``.cargo/config.toml`` target-dir = ``target``",
    "- Active build processes: $(@($processes).Count)",
    "- Lock evidence entries: $(@($lockEvidence).Count)",
    "- Clear preview exit code: $previewExit (no files deleted)",
    "",
    "| Path | Exists | Size (MB) |",
    "| --- | --- | ---: |"
)
foreach ($item in $report.target_paths) { $md += "| $($item.path) | $($item.exists) | $($item.megabytes) |" }
$md += "", "Raw process, lock, manifest and preview data: ``audit.json``."
$md -join "`r`n" | Set-Content -LiteralPath (Join-Path $runDirectory "audit.md") -Encoding UTF8
Write-Host "Shared cache audit: $(Join-Path $runDirectory 'audit.md')"
Write-Host "Raw audit: $jsonPath"
exit 0
