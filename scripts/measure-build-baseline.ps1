[CmdletBinding(PositionalBinding = $false)]
param(
    [ValidateSet("all", "environment", "desktop-cold", "desktop-warm", "desktop-incremental", "desktop-hot", "check", "ui-generation", "ui-audit", "headless", "android")]
    [string[]]$Scenario = @("all"),
    [switch]$Execute,
    [switch]$ExecuteAndroid,
    [switch]$IncludeStorageSnapshot,
    [string]$OutputDirectory = "artifacts/compile-baseline",
    [string]$ColdTargetDirectory,
    [switch]$ConfirmColdTarget,
    [int]$TimeoutSeconds = 1800,
    [string]$BaseSha = "9b5e61c261a13b49312303d3962afa0656432ffb"
)

$ErrorActionPreference = "Stop"

function Resolve-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Resolve-RepoPath {
    param([string]$RepoRoot, [string]$Path)
    if ([System.IO.Path]::IsPathRooted($Path)) {
        return [System.IO.Path]::GetFullPath($Path)
    }
    return [System.IO.Path]::GetFullPath((Join-Path $RepoRoot $Path))
}

function Test-PathInside {
    param([string]$Candidate, [string]$Container)
    $containerPath = [System.IO.Path]::GetFullPath($Container).TrimEnd([char[]]@('\', '/'))
    $candidatePath = [System.IO.Path]::GetFullPath($Candidate).TrimEnd([char[]]@('\', '/'))
    return $candidatePath.Equals($containerPath, [System.StringComparison]::OrdinalIgnoreCase) -or
        $candidatePath.StartsWith("$containerPath$([System.IO.Path]::DirectorySeparatorChar)", [System.StringComparison]::OrdinalIgnoreCase)
}

function Get-ColdTargetPrecondition {
    param([string]$RepoRoot, [string]$ArtifactsRoot, [string]$ColdTargetDirectory, [bool]$Confirmed)
    if ([string]::IsNullOrWhiteSpace($ColdTargetDirectory)) {
        return [pscustomobject]@{ ready = $false; path = $null; reason = "precondition unmet: pass -ColdTargetDirectory with a caller-created empty target under artifacts/ and -ConfirmColdTarget" }
    }
    $path = Resolve-RepoPath $RepoRoot $ColdTargetDirectory
    if (-not (Test-PathInside $path $ArtifactsRoot)) {
        return [pscustomobject]@{ ready = $false; path = $path; reason = "precondition unmet: ColdTargetDirectory must be inside ignored artifacts/" }
    }
    if (-not $Confirmed) {
        return [pscustomobject]@{ ready = $false; path = $path; reason = "precondition unmet: caller must pass -ConfirmColdTarget after verifying the target is safe and independent" }
    }
    if (-not (Test-Path -LiteralPath $path -PathType Container)) {
        return [pscustomobject]@{ ready = $false; path = $path; reason = "precondition unmet: ColdTargetDirectory must already exist and be empty; this script will not create or clear it" }
    }
    if (Get-ChildItem -LiteralPath $path -Force | Select-Object -First 1) {
        return [pscustomobject]@{ ready = $false; path = $path; reason = "precondition unmet: ColdTargetDirectory is not empty; this script will not clear it" }
    }
    return [pscustomobject]@{ ready = $true; path = $path; reason = "caller-confirmed empty independent target" }
}

function Get-SafeVersion {
    param([string]$FileName, [string[]]$Arguments = @())
    try {
        $output = & $FileName @Arguments 2>&1 | Out-String
        return $output.Trim()
    } catch {
        return "unavailable: $($_.Exception.Message)"
    }
}

function Get-DirectorySnapshot {
    param([string]$RepoRoot, [bool]$IncludeStorageSnapshot)
    if (-not $IncludeStorageSnapshot) {
        return [pscustomobject]@{
            sampled = $false
            reason = "Pass -IncludeStorageSnapshot to recursively measure target and incremental directories."
            directories = @()
            incremental = @()
        }
    }
    $paths = @(
        (Join-Path $RepoRoot "target"),
        (Join-Path $RepoRoot "project\target"),
        (Join-Path $RepoRoot "tools\ui-generation\target"),
        (Join-Path $RepoRoot "tools\ui-visual-audit\target"),
        (Join-Path $RepoRoot "android\app\build"),
        (Join-Path $RepoRoot "android\.gradle")
    )
    $incrementalBytes = @{}
    $rows = foreach ($path in $paths) {
        $exists = Test-Path -LiteralPath $path -PathType Container
        $bytes = [int64]0
        if ($exists) {
            try {
                foreach ($filePath in [System.IO.Directory]::EnumerateFiles($path, "*", [System.IO.SearchOption]::AllDirectories)) {
                    $length = [int64]([System.IO.FileInfo]::new($filePath)).Length
                    $bytes += $length
                    $marker = "\incremental\"
                    $index = $filePath.IndexOf($marker, [System.StringComparison]::OrdinalIgnoreCase)
                    if ($index -ge 0) {
                        $incrementalPath = $filePath.Substring(0, $index + $marker.Length - 1)
                        if (-not $incrementalBytes.ContainsKey($incrementalPath)) { $incrementalBytes[$incrementalPath] = [int64]0 }
                        $incrementalBytes[$incrementalPath] += $length
                    }
                }
            } catch {
                Write-Warning "Could not fully sample directory $($path): $($_.Exception.Message)"
            }
        }
        [pscustomobject]@{
            path = $path.Substring($RepoRoot.Length).TrimStart('\', '/')
            exists = $exists
            bytes = $bytes
            megabytes = [math]::Round($bytes / 1MB, 2)
        }
    }
    $incremental = foreach ($entry in $incrementalBytes.GetEnumerator() | Sort-Object Key) {
        [pscustomobject]@{
            path = $entry.Key.Substring($RepoRoot.Length).TrimStart('\', '/')
            bytes = $entry.Value
            megabytes = [math]::Round($entry.Value / 1MB, 2)
        }
    }
    return [pscustomobject]@{ directories = @($rows); incremental = @($incremental) }
}

function Get-ResourceSnapshot {
    param([string]$RepoRoot)
    $os = Get-CimInstance Win32_OperatingSystem -ErrorAction SilentlyContinue
    $cpu = Get-CimInstance Win32_Processor -ErrorAction SilentlyContinue | Select-Object -First 1
    $driveName = ([System.IO.Path]::GetPathRoot($RepoRoot)).TrimEnd('\').TrimEnd(':')
    $drive = Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='$driveName`:'" -ErrorAction SilentlyContinue
    $interesting = @("cargo", "rustc", "rustfmt", "link", "java", "gradle", "adb")
    $processes = Get-Process -ErrorAction SilentlyContinue |
        Where-Object { $interesting -contains $_.ProcessName.ToLowerInvariant() } |
        Select-Object ProcessName, Id, CPU, @{Name = "WorkingSetBytes"; Expression = { $_.WorkingSet64 }}
    return [pscustomobject]@{
        cpu = if ($cpu) { "$($cpu.Name) ($($cpu.NumberOfLogicalProcessors) logical processors)" } else { "unavailable" }
        memory_total_bytes = if ($os) { [int64]$os.TotalVisibleMemorySize * 1KB } else { $null }
        memory_free_bytes = if ($os) { [int64]$os.FreePhysicalMemory * 1KB } else { $null }
        disk_free_bytes = if ($drive) { [int64]$drive.FreeSpace } else { $null }
        processes = @($processes)
    }
}

function Quote-ProcessArgument {
    param([string]$Value)
    if ($Value -notmatch '[\s"]') { return $Value }
    return '"' + $Value.Replace('"', '\"') + '"'
}

function Invoke-MeasuredCommand {
    param(
        [string]$Name,
        [string]$FileName,
        [string[]]$Arguments,
        [string]$WorkingDirectory,
        [string]$LogPath,
        [int]$TimeoutSeconds,
        [switch]$Skip,
        [string]$SkipReason = "dry-run: pass -Execute to start this command"
    )
    $commandText = "$FileName " + (($Arguments | ForEach-Object { Quote-ProcessArgument $_ }) -join " ")
    $started = [DateTimeOffset]::Now
    if ($Skip) {
        Set-Content -LiteralPath $LogPath -Value "STATUS: SKIPPED`r`nREASON: $SkipReason`r`nCOMMAND: $commandText" -Encoding UTF8
        return [pscustomobject]@{ name = $Name; command = $commandText; status = "skipped"; exit_code = $null; elapsed_seconds = 0; started_at = $started.ToString("o"); finished_at = [DateTimeOffset]::Now.ToString("o"); log = $LogPath; cargo_lock_wait_detected = $null; skip_reason = $SkipReason }
    }

    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $FileName
    $psi.Arguments = (($Arguments | ForEach-Object { Quote-ProcessArgument $_ }) -join " ")
    $psi.WorkingDirectory = $WorkingDirectory
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $psi
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $stdout = ""
    $stderr = ""
    $status = "failed"
    $exitCode = $null
    try {
        if (-not $process.Start()) { throw "could not start process" }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            $status = "timeout"
            try { $process.Kill() } catch { }
        } else {
            $exitCode = $process.ExitCode
            $status = if ($exitCode -eq 0) { "passed" } else { "failed" }
        }
        $stdout = $stdoutTask.Result
        $stderr = $stderrTask.Result
    } catch {
        $stderr = $_.Exception.ToString()
    } finally {
        $timer.Stop()
        $process.Dispose()
    }
    $log = "COMMAND: $commandText`r`nEXIT_CODE: $exitCode`r`nSTATUS: $status`r`nELAPSED_SECONDS: $([math]::Round($timer.Elapsed.TotalSeconds, 3))`r`n`r`n--- STDOUT ---`r`n$stdout`r`n--- STDERR ---`r`n$stderr"
    Set-Content -LiteralPath $LogPath -Value $log -Encoding UTF8
    $lockWait = $log -match '(?i)(blocking )?waiting for file lock|could not acquire lock'
    return [pscustomobject]@{ name = $Name; command = $commandText; status = $status; exit_code = $exitCode; elapsed_seconds = [math]::Round($timer.Elapsed.TotalSeconds, 3); started_at = $started.ToString("o"); finished_at = [DateTimeOffset]::Now.ToString("o"); log = $LogPath; cargo_lock_wait_detected = $lockWait }
}

function Get-ArtifactSnapshot {
    param([string]$RepoRoot)
    $candidatePaths = @(
        (Join-Path $RepoRoot "target\debug\project.exe"),
        (Join-Path $RepoRoot "target\release\project.exe"),
        (Join-Path $RepoRoot "target\debug\lockstep-sim-headless.exe"),
        (Join-Path $RepoRoot "target\debug\fangyuan_bake.exe"),
        (Join-Path $RepoRoot "android\app\build\outputs\apk\debug\app-debug.apk"),
        (Join-Path $RepoRoot "android\app\src\main\jniLibs\arm64-v8a\libproject.so")
    )
    return @($candidatePaths | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | ForEach-Object {
        $item = Get-Item -LiteralPath $_
        [pscustomobject]@{ path = $_.Substring($RepoRoot.Length).TrimStart('\', '/'); bytes = [int64]$item.Length; megabytes = [math]::Round($item.Length / 1MB, 2); last_write_time = $item.LastWriteTime.ToString("o") }
    })
}

$scriptStartedAt = [DateTimeOffset]::Now
$repoRoot = Resolve-RepoRoot
$outputRoot = Resolve-RepoPath $repoRoot $OutputDirectory
$allowedRoot = Resolve-RepoPath $repoRoot "artifacts"
if (-not (Test-PathInside $outputRoot $allowedRoot)) {
    throw "OutputDirectory must remain under ignored artifacts/: $outputRoot"
}
$runId = [DateTimeOffset]::Now.ToString("yyyyMMdd-HHmmss")
$runDirectory = Join-Path $outputRoot $runId
New-Item -ItemType Directory -Path $runDirectory -Force | Out-Null

$allScenarios = @("desktop-cold", "desktop-warm", "desktop-incremental", "desktop-hot", "check", "ui-generation", "ui-audit", "headless", "android")
$selected = if ($Scenario -contains "all") { $allScenarios } else { @($Scenario | Where-Object { $_ -ne "environment" }) }
$collectToolVersions = [bool]$Execute
$gitSha = if ($collectToolVersions) { Get-SafeVersion "git" @("rev-parse", "HEAD") } else { "not collected in dry-run" }
$manifestText = Get-Content (Join-Path $repoRoot "project\Cargo.toml") -Raw
$bevyVersion = if ($manifestText -match 'bevy\s*=\s*\{[^}]*version\s*=\s*"([^"]+)"') { $Matches[1] } else { "unknown" }
$gradleWrapperProperties = Join-Path $repoRoot "android\gradle\wrapper\gradle-wrapper.properties"
$gradleDistribution = if (Test-Path -LiteralPath $gradleWrapperProperties) {
    (Select-String -LiteralPath $gradleWrapperProperties -Pattern '^distributionUrl=' | Select-Object -First 1).Line
} else { "gradle wrapper properties unavailable" }
$environment = [pscustomobject]@{
    timestamp = [DateTimeOffset]::Now.ToString("o")
    repository = $repoRoot
    base_sha_expected = $BaseSha
    git_sha = $gitSha
    os = if ($collectToolVersions) { Get-SafeVersion "cmd.exe" @("/c", "ver") } else { [System.Environment]::OSVersion.VersionString }
    powershell = $PSVersionTable.PSVersion.ToString()
    rustc = if ($collectToolVersions) { Get-SafeVersion "rustc" @("--version", "--verbose") } else { "not collected in dry-run" }
    cargo = if ($collectToolVersions) { Get-SafeVersion "cargo" @("--version", "--verbose") } else { "not collected in dry-run" }
    rustup = if ($collectToolVersions) { Get-SafeVersion "rustup" @("show", "active-toolchain") } else { "not collected in dry-run" }
    ndk = if ($env:ANDROID_NDK_HOME) { $env:ANDROID_NDK_HOME } else { "ANDROID_NDK_HOME not set" }
    java = if ($collectToolVersions) {
        if ($env:JAVA_HOME) { "$env:JAVA_HOME`n$(Get-SafeVersion 'java' @('-version'))" } else { Get-SafeVersion "java" @("-version") }
    } else { "not collected in dry-run" }
    gradle_wrapper = $gradleDistribution
    bevy = $bevyVersion
    resource = Get-ResourceSnapshot $repoRoot
    directories = Get-DirectorySnapshot $repoRoot $IncludeStorageSnapshot
}

$results = [System.Collections.Generic.List[object]]::new()
if (-not $Execute) {
    Write-Host "Dry-run: no external Cargo, Android, Gradle, Java, or version command will run. Use -Execute for selected scenarios."
}

$projectRoot = Join-Path $repoRoot "project"
$coldPrecondition = Get-ColdTargetPrecondition $repoRoot $allowedRoot $ColdTargetDirectory $ConfirmColdTarget
$commands = @{
    "desktop-cold" = @{ file = "cargo"; args = @("build", "--locked"); cwd = $projectRoot; note = "Cold baseline requires a caller-confirmed empty independent target under artifacts/." }
    "desktop-warm" = @{ file = "cargo"; args = @("build", "--locked"); cwd = $projectRoot; note = "Warm desktop build against the current existing Cargo target." }
    "desktop-incremental" = @{ file = "cargo"; args = @("build", "--locked"); cwd = $projectRoot; note = "Not measured automatically: copy or patch a caller-selected ordinary Rust file outside this script, then run this scenario and restore the file separately." }
    "desktop-hot" = @{ file = "cargo"; args = @("build", "--locked"); cwd = $projectRoot; note = "Not measured automatically: copy or patch a caller-selected high-churn Rust file outside this script, then run this scenario and restore the file separately." }
    "check" = @{ file = "cargo"; args = @("check", "--locked"); cwd = $projectRoot; note = "Records compiler failure stage and lock waits." }
    "ui-generation" = @{ file = "cargo"; args = @("build", "--locked"); cwd = (Join-Path $repoRoot "tools\ui-generation"); note = "UI generation tool build." }
    "ui-audit" = @{ file = "cargo"; args = @("build", "--locked"); cwd = (Join-Path $repoRoot "tools\ui-visual-audit"); note = "UI visual audit tool build." }
    "headless" = @{ file = "cargo"; args = @("build", "--locked", "--bin", "lockstep-sim-headless"); cwd = $projectRoot; note = "Headless simulation binary build." }
}

foreach ($name in $selected) {
    if ($name -eq "desktop-cold") {
        $coldLog = Join-Path $runDirectory "desktop-cold.log"
        if (-not $coldPrecondition.ready) {
            $results.Add((Invoke-MeasuredCommand -Name "desktop-cold" -FileName "cargo" -Arguments $commands[$name].args -WorkingDirectory $projectRoot -LogPath $coldLog -TimeoutSeconds $TimeoutSeconds -Skip -SkipReason $coldPrecondition.reason))
            continue
        }
        $coldArgs = @("build", "--locked", "--target-dir", $coldPrecondition.path)
        $results.Add((Invoke-MeasuredCommand -Name "desktop-cold" -FileName "cargo" -Arguments $coldArgs -WorkingDirectory $projectRoot -LogPath $coldLog -TimeoutSeconds $TimeoutSeconds -Skip:(-not [bool]$Execute) -SkipReason "dry-run: caller-confirmed cold target command planned but not started"))
        continue
    }
    if ($name -eq "desktop-incremental" -or $name -eq "desktop-hot") {
        $workflowLog = Join-Path $runDirectory "$name.log"
        $results.Add((Invoke-MeasuredCommand -Name $name -FileName $commands[$name].file -Arguments $commands[$name].args -WorkingDirectory $commands[$name].cwd -LogPath $workflowLog -TimeoutSeconds $TimeoutSeconds -Skip -SkipReason "not measured: $($commands[$name].note)"))
        continue
    }
    if ($name -eq "android") {
        $androidLog = Join-Path $runDirectory "android-rust-release.log"
        $androidArgs = @("ndk", "-t", "arm64-v8a", "-P", "26", "-o", (Join-Path $repoRoot "android\app\src\main\jniLibs"), "rustc", "--release", "--lib", "--crate-type", "cdylib")
        $gradleLog = Join-Path $runDirectory "android-gradle-assemble-debug.log"
        if (-not $Execute -or -not $ExecuteAndroid) {
            $reason = if (-not $Execute) { "dry-run: Android Rust release command planned but not started" } else { "Android measurement skipped: pass -ExecuteAndroid only after target isolation and explicit approval" }
            $gradleReason = if (-not $Execute) { "dry-run: Android Gradle command planned but not started" } else { "Android measurement skipped: pass -ExecuteAndroid only after target isolation and explicit approval" }
            $results.Add((Invoke-MeasuredCommand -Name "android-rust-release" -FileName "cargo" -Arguments $androidArgs -WorkingDirectory $projectRoot -LogPath $androidLog -TimeoutSeconds $TimeoutSeconds -Skip -SkipReason $reason))
            $results.Add((Invoke-MeasuredCommand -Name "android-gradle-assemble-debug" -FileName (Join-Path $repoRoot "android\gradlew.bat") -Arguments @("assembleDebug") -WorkingDirectory (Join-Path $repoRoot "android") -LogPath $gradleLog -TimeoutSeconds $TimeoutSeconds -Skip -SkipReason $gradleReason))
            continue
        }
        $ndkReady = [bool](Get-Command cargo -ErrorAction SilentlyContinue) -and [bool](Get-Command java -ErrorAction SilentlyContinue) -and (Test-Path (Join-Path $repoRoot "android\gradlew.bat"))
        $cargoNdkReady = (Get-SafeVersion "cargo" @("ndk", "--version")) -notmatch "unavailable|error|could not"
        if (-not $ndkReady -or -not $cargoNdkReady) {
            $results.Add([pscustomobject]@{ name = "android"; status = "skipped"; note = "cargo-ndk, Java, or Gradle wrapper unavailable; no Android command started."; command = "cargo ndk ...; android\gradlew.bat assembleDebug" })
            continue
        }
        $results.Add((Invoke-MeasuredCommand -Name "android-rust-release" -FileName "cargo" -Arguments $androidArgs -WorkingDirectory $projectRoot -LogPath $androidLog -TimeoutSeconds $TimeoutSeconds))
        $results.Add((Invoke-MeasuredCommand -Name "android-gradle-assemble-debug" -FileName (Join-Path $repoRoot "android\gradlew.bat") -Arguments @("assembleDebug") -WorkingDirectory (Join-Path $repoRoot "android") -LogPath $gradleLog -TimeoutSeconds $TimeoutSeconds))
        continue
    }
    $spec = $commands[$name]
    $logPath = Join-Path $runDirectory "$name.log"
    $result = Invoke-MeasuredCommand -Name $name -FileName $spec.file -Arguments $spec.args -WorkingDirectory $spec.cwd -LogPath $logPath -TimeoutSeconds $TimeoutSeconds -Skip:(-not [bool]$Execute)
    $result | Add-Member -NotePropertyName note -NotePropertyValue $spec.note
    $results.Add($result)
}

$report = [pscustomobject]@{
    schema = "mybevy.compile-baseline.v1"
    dry_run = -not $Execute
    started_at = $scriptStartedAt.ToString("o")
    scenarios = @($selected)
    cold_precondition = $coldPrecondition
    command_plan = @($results | ForEach-Object { [pscustomobject]@{ name = $_.name; command = $_.command; status = $_.status; started_at = $_.started_at; finished_at = $_.finished_at; exit_code = $_.exit_code; cargo_lock_wait_detected = $_.cargo_lock_wait_detected; log = $_.log; skip_reason = $_.skip_reason } })
    environment = $environment
    before = [pscustomobject]@{ resources = $environment.resource; directories = $environment.directories; artifacts = @(Get-ArtifactSnapshot $repoRoot) }
    commands = @($results)
    after = [pscustomobject]@{ resources = Get-ResourceSnapshot $repoRoot; directories = Get-DirectorySnapshot $repoRoot $IncludeStorageSnapshot; artifacts = @(Get-ArtifactSnapshot $repoRoot) }
    finished_at = [DateTimeOffset]::Now.ToString("o")
}
$jsonPath = Join-Path $runDirectory "report.json"
$markdownPath = Join-Path $runDirectory "report.md"
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $jsonPath -Encoding UTF8

$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add("# Compile baseline $runId")
$lines.Add("")
$lines.Add("- Dry run: **$(-not $Execute)**")
$lines.Add(('- Git SHA: `{0}`' -f $gitSha))
$lines.Add(('- Expected base SHA: `{0}`' -f $BaseSha))
$lines.Add(('- Bevy: `{0}`' -f $bevyVersion))
$lines.Add("")
$lines.Add("| Scenario | Status | Elapsed (s) | Exit code | Lock wait |")
$lines.Add("| --- | --- | ---: | ---: | --- |")
foreach ($item in $results) {
    $elapsed = if ($null -eq $item.elapsed_seconds) { "" } else { $item.elapsed_seconds }
    $exit = if ($null -eq $item.exit_code) { "" } else { $item.exit_code }
    $lock = if ($null -eq $item.cargo_lock_wait_detected) { "" } else { $item.cargo_lock_wait_detected }
    $lines.Add("| $($item.name) | $($item.status) | $elapsed | $exit | $lock |")
}
$lines.Add("")
$lines.Add('Raw command logs are stored beside this report. Directory sizes and resource samples are in `report.json`.')
$lines -join "`r`n" | Set-Content -LiteralPath $markdownPath -Encoding UTF8

Write-Host "Baseline report: $markdownPath"
Write-Host "Machine/environment report: $jsonPath"
if ($Execute) {
    $failed = @($results | Where-Object { $_.status -eq "failed" -or $_.status -eq "timeout" })
    if ($failed.Count -gt 0) { exit 1 }
}
exit 0
