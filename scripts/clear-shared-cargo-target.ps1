[CmdletBinding()]
param(
    [switch]$Execute,
    [switch]$ConfirmSharedTargetCleanup,
    [switch]$IncrementalOnly,
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"

function Get-NormalizedPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    return [System.IO.Path]::GetFullPath($Path).TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
}

function Test-SamePath {
    param([Parameter(Mandatory = $true)][string]$Left, [Parameter(Mandatory = $true)][string]$Right)
    return [string]::Equals((Get-NormalizedPath $Left), (Get-NormalizedPath $Right), [System.StringComparison]::OrdinalIgnoreCase)
}

function Test-PathWithin {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$Parent)
    $child = Get-NormalizedPath $Path
    $root = Get-NormalizedPath $Parent
    return $child.StartsWith($root + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)
}

$repoRoot = Get-NormalizedPath (Join-Path $PSScriptRoot "..")
$legacyTarget = Get-NormalizedPath (Join-Path $repoRoot "target")
$cleanupScope = if ($IncrementalOnly) { Get-NormalizedPath (Join-Path $legacyTarget "debug\incremental") } else { $legacyTarget }

if (-not (Test-SamePath $legacyTarget (Join-Path $repoRoot "target")) -or -not (Test-PathWithin $legacyTarget $repoRoot)) {
    throw "Refusing cleanup because legacy target does not resolve to the repository root target."
}
if (-not (Test-SamePath $cleanupScope $legacyTarget) -and -not (Test-SamePath $cleanupScope (Join-Path $legacyTarget "debug\incremental"))) {
    throw "Refusing cleanup because requested scope is not the legacy root target or its debug incremental directory."
}

function Test-ReparsePoint {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return $false }
    return [bool]((Get-Item -LiteralPath $Path -Force).Attributes -band [System.IO.FileAttributes]::ReparsePoint)
}

function Get-ProcessSnapshot {
    try {
        return @(Get-CimInstance -ClassName Win32_Process -ErrorAction Stop | ForEach-Object {
            [pscustomobject]@{
                Id = $_.ProcessId
                Name = $_.Name
                ExecutablePath = $_.ExecutablePath
                CommandLine = $_.CommandLine
            }
        })
    }
    catch {
        return @(Get-Process -ErrorAction SilentlyContinue | ForEach-Object {
            [pscustomobject]@{
                Id = $_.Id
                Name = $_.ProcessName
                ExecutablePath = $_.Path
                CommandLine = ""
            }
        })
    }
}

function Get-BlockingProcessReason {
    param([Parameter(Mandatory = $true)][pscustomobject]$Process)

    $name = [System.IO.Path]::GetFileNameWithoutExtension([string]$Process.Name).ToLowerInvariant()
    $executablePath = [string]$Process.ExecutablePath
    $commandLine = [string]$Process.CommandLine

    if ($name -in @("cargo", "rustc", "rustdoc", "link")) { return "Cargo, Rust compiler, or linker" }
    if ($name -in @("project", "ui-document-preview", "ui_document_preview", "ui-generation", "ui_generation", "ui-visual-audit", "ui_visual_audit")) { return "game or UI tool" }
    if ($name -in @("gradle", "gradlew", "adb")) { return "Android or Gradle build tooling" }

    if ($name -in @("java", "javaw")) {
        $markers = @($repoRoot, "gradle", "com.android.build", "android")
        foreach ($marker in $markers) {
            if ($commandLine.IndexOf($marker, [System.StringComparison]::OrdinalIgnoreCase) -ge 0 -or
                $executablePath.IndexOf($marker, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
                return "Java process associated with an Android or Gradle build"
            }
        }
    }

    return $null
}

function Get-BlockingProcesses {
    return @(Get-ProcessSnapshot | ForEach-Object {
        $reason = Get-BlockingProcessReason $_
        if ($null -ne $reason) {
            [pscustomobject]@{ Id = $_.Id; ProcessName = $_.Name; Reason = $reason }
        }
    })
}

if ($SelfTest) {
    if (-not (Test-PathWithin (Join-Path $repoRoot "target\debug") $repoRoot)) { throw "Self-test failed: root target is inside repository" }
    if (Test-PathWithin (Join-Path $repoRoot "target-sibling\debug") $legacyTarget) { throw "Self-test failed: target-sibling prefix bypass" }
    foreach ($path in @("project\target", "tools\ui-generation\target", "tools\ui-visual-audit\target", "project\target-android")) {
        if (Test-SamePath (Join-Path $repoRoot $path) $cleanupScope) { throw "Self-test failed: $path accepted as legacy target" }
    }
    $fakeGradleJava = [pscustomobject]@{ Id = 1; Name = "java.exe"; ExecutablePath = "C:\\Program Files\\Java\\jdk-21\\bin\\java.exe"; CommandLine = "org.gradle.launcher.daemon.bootstrap.GradleDaemon" }
    $fakeUnrelatedJava = [pscustomobject]@{ Id = 2; Name = "javaw.exe"; ExecutablePath = "C:\\Program Files\\Java\\jdk-21\\bin\\javaw.exe"; CommandLine = "com.example.service.Main" }
    if ($null -eq (Get-BlockingProcessReason $fakeGradleJava)) { throw "Self-test failed: Gradle Java process is not blocked" }
    if ($null -ne (Get-BlockingProcessReason $fakeUnrelatedJava)) { throw "Self-test failed: unrelated Java process is blocked" }
    Write-Host "Legacy shared Cargo target cleanup self-test passed."
    exit 0
}

$blocking = @(Get-BlockingProcesses)
if ($blocking.Count -gt 0) {
    $details = ($blocking | ForEach-Object { "PID $($_.Id) $($_.ProcessName): $($_.Reason)" }) -join [Environment]::NewLine
    throw "Legacy target cleanup is blocked by active build processes:$([Environment]::NewLine)$details"
}

$mode = if ($IncrementalOnly) { "legacy root debug incremental cache" } else { "legacy shared root target" }
if (-not $Execute) {
    Write-Host "Dry run only. No files will be deleted."
    Write-Host "Would clean ${mode}: $cleanupScope"
    Write-Host "New isolated targets are never cleanup targets: project/target, project/target-android, tools/*/target."
    exit 0
}
if (-not $ConfirmSharedTargetCleanup) { throw "Refusing deletion without -ConfirmSharedTargetCleanup. The default mode is dry run." }
if ((Test-ReparsePoint $cleanupScope) -or ((Test-SamePath $cleanupScope $legacyTarget) -and (Test-ReparsePoint (Join-Path $legacyTarget "debug")))) {
    throw "Refusing cleanup because a cleanup path is a reparse point: $cleanupScope"
}
if (-not (Test-Path -LiteralPath $cleanupScope -PathType Container)) {
    Write-Host "Nothing to clean. Directory does not exist: $cleanupScope"
    exit 0
}
[System.IO.Directory]::Delete($cleanupScope, $true)
Write-Host "Legacy shared Cargo target cleanup completed: $cleanupScope"
