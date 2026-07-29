[CmdletBinding()]
param(
    [switch]$Execute,
    [switch]$ConfirmSharedTargetCleanup,
    [switch]$IncrementalOnly,
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"

function Get-NormalizedPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    return [System.IO.Path]::GetFullPath($Path).TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
}

function Test-SamePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Left,
        [Parameter(Mandatory = $true)]
        [string]$Right
    )

    return [string]::Equals((Get-NormalizedPath -Path $Left), (Get-NormalizedPath -Path $Right), [System.StringComparison]::OrdinalIgnoreCase)
}

function Test-PathWithin {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Parent
    )

    $normalizedPath = Get-NormalizedPath -Path $Path
    $normalizedParent = Get-NormalizedPath -Path $Parent
    $prefix = $normalizedParent + [System.IO.Path]::DirectorySeparatorChar
    return $normalizedPath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)
}

function Get-SharedTargetContext {
    $scriptRoot = Get-NormalizedPath -Path (Split-Path -Parent $PSCommandPath)
    $repoRoot = Get-NormalizedPath -Path (Join-Path $scriptRoot "..")
    $sharedTarget = Get-NormalizedPath -Path (Join-Path $repoRoot "target")
    $expectedTarget = Get-NormalizedPath -Path (Join-Path $repoRoot "target")
    $configPath = Join-Path $repoRoot ".cargo/config.toml"

    if (-not (Test-SamePath -Left $sharedTarget -Right $expectedTarget)) {
        throw "Refusing cleanup because the resolved target is not the repository root target."
    }

    if (-not (Test-PathWithin -Path $sharedTarget -Parent $repoRoot)) {
        throw "Refusing cleanup because the resolved target is outside the repository root."
    }

    if (-not (Test-Path -LiteralPath $configPath -PathType Leaf)) {
        throw "Refusing cleanup because the shared Cargo configuration is missing: $configPath"
    }

    $config = Get-Content -LiteralPath $configPath -Raw
    if ($config -notmatch '(?m)^\s*target-dir\s*=\s*"target"\s*(?:#.*)?$') {
        throw 'Refusing cleanup because .cargo/config.toml does not declare target-dir = "target".'
    }

    return [pscustomobject]@{
        RepoRoot = $repoRoot
        SharedTarget = $sharedTarget
    }
}

function Get-CleanupScope {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Context,
        [Parameter(Mandatory = $true)]
        [bool]$UseIncrementalOnly
    )

    if ($UseIncrementalOnly) {
        return Get-NormalizedPath -Path (Join-Path $Context.SharedTarget "debug/incremental")
    }

    return $Context.SharedTarget
}

function Assert-ApprovedCleanupScope {
    param(
        [Parameter(Mandatory = $true)]
        [string]$CleanupScope,
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Context
    )

    $incrementalScope = Get-NormalizedPath -Path (Join-Path $Context.SharedTarget "debug/incremental")
    if ((Test-SamePath -Left $CleanupScope -Right $Context.SharedTarget) -or (Test-SamePath -Left $CleanupScope -Right $incrementalScope)) {
        return
    }

    throw "Refusing cleanup because the requested scope is not the shared target or its debug incremental cache: $CleanupScope"
}

function Test-ReparsePoint {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return $false
    }

    $item = Get-Item -LiteralPath $Path -Force
    return [bool]($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint)
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
        return @(Get-Process | ForEach-Object {
            [pscustomobject]@{
                Id = $_.Id
                Name = $_.ProcessName
                ExecutablePath = ""
                CommandLine = ""
            }
        })
    }
}

function Get-BlockingProcessReason {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Process,
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Context
    )

    $name = [System.IO.Path]::GetFileNameWithoutExtension([string]$Process.Name).ToLowerInvariant()
    $executablePath = [string]$Process.ExecutablePath
    $commandLine = [string]$Process.CommandLine
    $pathInSharedTarget = $false

    if (-not [string]::IsNullOrWhiteSpace($executablePath)) {
        try {
            $pathInSharedTarget = Test-PathWithin -Path $executablePath -Parent $Context.SharedTarget
        }
        catch {
            $pathInSharedTarget = $false
        }
    }

    if ($name -in @("cargo", "rustc", "link")) {
        return "Cargo, Rust compiler, or linker"
    }

    if ($name -in @("project", "ui-document-preview", "ui_document_preview", "ui-generation", "ui_generation", "ui-visual-audit", "ui_visual_audit")) {
        return "game or UI tool"
    }

    if (($name -like "project-*") -or ($name -like "ui_generation-*") -or ($name -like "ui-generation-*") -or ($name -like "ui_visual_audit-*") -or ($name -like "ui-visual-audit-*")) {
        if ($pathInSharedTarget -or [string]::IsNullOrWhiteSpace($executablePath)) {
            return "test executable or UI tool child process"
        }
    }

    if ($name -in @("gradle", "gradlew", "adb")) {
        return "Android or Gradle build tooling"
    }

    if ($name -in @("java", "javaw")) {
        $androidMarkers = @($Context.RepoRoot, "gradle", "com.android.build", "android")
        foreach ($marker in $androidMarkers) {
            if (-not [string]::IsNullOrWhiteSpace($marker) -and $commandLine.IndexOf($marker, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
                return "Java process associated with an Android or Gradle build"
            }
        }
    }

    return $null
}

function Get-BlockingProcesses {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Context
    )

    $blocking = @()
    foreach ($process in (Get-ProcessSnapshot)) {
        $reason = Get-BlockingProcessReason -Process $process -Context $Context
        if ($null -ne $reason) {
            $blocking += [pscustomobject]@{
                Id = $process.Id
                Name = $process.Name
                Reason = $reason
            }
        }
    }

    return $blocking
}

function Assert-NoBlockingProcesses {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Context
    )

    $blocking = @(Get-BlockingProcesses -Context $Context)
    if ($blocking.Count -eq 0) {
        return
    }

    $details = ($blocking | ForEach-Object { "PID $($_.Id) $($_.Name): $($_.Reason)" }) -join [Environment]::NewLine
    throw "Shared target cleanup is blocked by active processes:$([Environment]::NewLine)$details"
}

function Assert-NoReparsePointOnCleanupPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$CleanupScope,
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Context
    )

    $pathsToCheck = @($Context.SharedTarget)
    if (-not (Test-SamePath -Left $CleanupScope -Right $Context.SharedTarget)) {
        $pathsToCheck += Get-NormalizedPath -Path (Join-Path $Context.SharedTarget "debug")
        $pathsToCheck += $CleanupScope
    }

    foreach ($path in $pathsToCheck) {
        if (Test-ReparsePoint -Path $path) {
            throw "Refusing cleanup because a cleanup path is a reparse point: $path"
        }
    }
}

function Assert-SelfTest {
    param(
        [Parameter(Mandatory = $true)]
        [bool]$Condition,
        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    if (-not $Condition) {
        throw "Self-test failed: $Message"
    }
}

function Test-ExpectedFailure {
    param(
        [Parameter(Mandatory = $true)]
        [scriptblock]$Action
    )

    try {
        & $Action
    }
    catch {
        return $true
    }

    return $false
}

$context = Get-SharedTargetContext
$cleanupScope = Get-CleanupScope -Context $context -UseIncrementalOnly ([bool]$IncrementalOnly)
Assert-ApprovedCleanupScope -CleanupScope $cleanupScope -Context $context

if ($SelfTest) {
    Assert-SelfTest -Condition (Test-SamePath -Left $context.SharedTarget -Right (Join-Path $context.RepoRoot "target")) -Message "shared target resolves to the repository target"
    Assert-SelfTest -Condition (Test-PathWithin -Path $context.SharedTarget -Parent $context.RepoRoot) -Message "shared target remains inside the repository"
    Assert-SelfTest -Condition (Test-ExpectedFailure -Action { Assert-ApprovedCleanupScope -CleanupScope (Join-Path $context.RepoRoot "project/target") -Context $context }) -Message "project target is rejected"
    Assert-SelfTest -Condition (Test-ExpectedFailure -Action { Assert-ApprovedCleanupScope -CleanupScope (Join-Path $context.RepoRoot "tools/ui-generation/target") -Context $context }) -Message "ui-generation target is rejected"
    Assert-SelfTest -Condition (Test-ExpectedFailure -Action { Assert-ApprovedCleanupScope -CleanupScope (Join-Path $context.RepoRoot "tools/ui-visual-audit/target") -Context $context }) -Message "ui-visual-audit target is rejected"

    $fakeCargo = [pscustomobject]@{ Id = 1; Name = "cargo.exe"; ExecutablePath = ""; CommandLine = "" }
    $fakeGradleJava = [pscustomobject]@{ Id = 2; Name = "java.exe"; ExecutablePath = ""; CommandLine = "org.gradle.launcher.daemon.bootstrap.GradleDaemon" }
    $fakeGame = [pscustomobject]@{ Id = 3; Name = "project.exe"; ExecutablePath = ""; CommandLine = "" }
    $fakeUiGeneration = [pscustomobject]@{ Id = 4; Name = "ui-generation.exe"; ExecutablePath = ""; CommandLine = "" }
    $fakeUiAudit = [pscustomobject]@{ Id = 5; Name = "ui-visual-audit.exe"; ExecutablePath = ""; CommandLine = "" }
    $fakeAdb = [pscustomobject]@{ Id = 6; Name = "adb.exe"; ExecutablePath = ""; CommandLine = "" }
    $unrelatedJava = [pscustomobject]@{ Id = 7; Name = "java.exe"; ExecutablePath = ""; CommandLine = "com.example.service.Main" }
    Assert-SelfTest -Condition ($null -ne (Get-BlockingProcessReason -Process $fakeCargo -Context $context)) -Message "cargo process is blocked"
    Assert-SelfTest -Condition ($null -ne (Get-BlockingProcessReason -Process $fakeGradleJava -Context $context)) -Message "Gradle Java process is blocked"
    Assert-SelfTest -Condition ($null -ne (Get-BlockingProcessReason -Process $fakeGame -Context $context)) -Message "game process is blocked"
    Assert-SelfTest -Condition ($null -ne (Get-BlockingProcessReason -Process $fakeUiGeneration -Context $context)) -Message "ui-generation process is blocked"
    Assert-SelfTest -Condition ($null -ne (Get-BlockingProcessReason -Process $fakeUiAudit -Context $context)) -Message "ui-visual-audit process is blocked"
    Assert-SelfTest -Condition ($null -ne (Get-BlockingProcessReason -Process $fakeAdb -Context $context)) -Message "ADB process is blocked"
    Assert-SelfTest -Condition ($null -eq (Get-BlockingProcessReason -Process $unrelatedJava -Context $context)) -Message "unrelated Java process is not treated as an Android build"
    Write-Host "Shared Cargo target cleanup self-test passed."
    exit 0
}

Assert-NoBlockingProcesses -Context $context

$mode = if ($IncrementalOnly) { "stale debug incremental cache" } else { "entire shared target" }
if (-not $Execute) {
    Write-Host "Dry run only. No files will be deleted."
    Write-Host "Would clean ${mode}: $cleanupScope"
    Write-Host "To delete after stopping all listed tooling, rerun with -Execute -ConfirmSharedTargetCleanup."
    exit 0
}

if (-not $ConfirmSharedTargetCleanup) {
    throw "Refusing deletion without -ConfirmSharedTargetCleanup. The default mode is dry run."
}

Assert-NoBlockingProcesses -Context $context
Assert-NoReparsePointOnCleanupPath -CleanupScope $cleanupScope -Context $context

if (-not (Test-Path -LiteralPath $cleanupScope -PathType Container)) {
    Write-Host "Nothing to clean. Directory does not exist: $cleanupScope"
    exit 0
}

Write-Host "Deleting ${mode}: $cleanupScope"
[System.IO.Directory]::Delete($cleanupScope, $true)
Write-Host "Shared Cargo target cleanup completed."
