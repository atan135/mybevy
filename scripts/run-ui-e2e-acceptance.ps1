[CmdletBinding(PositionalBinding = $false)]
param(
    [string]$RunId = "",
    [switch]$SkipDesktopRunner,
    [switch]$SkipRunnerSelfTest,
    [string]$ExistingDesktopManifest = "",
    [string]$RunnerSelfTestReceipt = ""
)

$ErrorActionPreference = "Stop"

function Get-Stage11FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    return [System.IO.Path]::GetFullPath($Path)
}

function ConvertFrom-Stage11ExtendedPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    return ($Path -replace '^[\\]{2}[?][\\]', '')
}

function Write-Stage11Json {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )

    $Value | ConvertTo-Json -Depth 64 | Set-Content -LiteralPath $Path -Encoding UTF8
}

function Invoke-Stage11Command {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Program,
        [Parameter(Mandatory = $true)][string[]]$ArgumentList,
        [Parameter(Mandatory = $true)][string]$LogPath,
        [switch]$ExpectJson
    )

    $started = [System.DateTimeOffset]::UtcNow
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    # Native tools can emit harmless compiler/linker diagnostics on stderr. Their documented
    # exit code, not PowerShell's NativeCommandError promotion, determines command success.
    $savedErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        $stdout = @(& $Program @ArgumentList 2> $LogPath)
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $savedErrorActionPreference
    }
    $watch.Stop()
    $record = [ordered]@{
        name = $Name
        program = $Program
        arguments = $ArgumentList
        started_utc = $started.ToString("o")
        elapsed_ms = [int64]$watch.ElapsedMilliseconds
        exit_code = $exitCode
        log = $LogPath
    }
    if ($exitCode -ne 0) {
        Write-Stage11Json -Path ($LogPath + ".command.json") -Value $record
        throw "Stage 11 command failed: $Name (exit $exitCode). See $LogPath"
    }
    if ($ExpectJson) {
        $raw = ($stdout -join "`n").Trim()
        if ([string]::IsNullOrWhiteSpace($raw)) {
            throw "Stage 11 command returned no JSON: $Name"
        }
        try {
            $record["json"] = $raw | ConvertFrom-Json
        } catch {
            throw "Stage 11 command returned malformed JSON: $Name. See $LogPath"
        }
    }
    Write-Stage11Json -Path ($LogPath + ".command.json") -Value $record
    return [pscustomobject]$record
}

function New-Stage11Task {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureTask,
        [Parameter(Mandatory = $true)][string]$RunIdValue,
        [Parameter(Mandatory = $true)][string]$TemporaryRoot,
        [Parameter(Mandatory = $true)][string]$RepositoryRoot
    )

    $fixtureDirectory = Split-Path -Parent $FixtureTask
    $task = (Get-Content -LiteralPath $FixtureTask -Raw | ConvertFrom-Json)
    $task.run_id = $RunIdValue
    $task.primary_reference.path = "reference.png"
    $taskDirectory = Join-Path $TemporaryRoot $RunIdValue
    New-Item -ItemType Directory -Force -Path $taskDirectory | Out-Null
    Copy-Item -LiteralPath (Join-Path $fixtureDirectory "reference.png") -Destination (Join-Path $taskDirectory "reference.png")
    $taskPath = Join-Path $taskDirectory "task.json"
    $taskJson = $task | ConvertTo-Json -Depth 32
    [System.IO.File]::WriteAllText($taskPath, $taskJson + [Environment]::NewLine, (New-Object System.Text.UTF8Encoding($false)))
    return $taskPath
}

function Invoke-Stage11IdentityReferenceCheck {
    param(
        [Parameter(Mandatory = $true)]$Audit,
        [Parameter(Mandatory = $true)][string]$RunRoot,
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$LogDirectory
    )

    $results = New-Object System.Collections.Generic.List[object]
    $outputParent = Join-Path $RunRoot "reference-integrity"
    New-Item -ItemType Directory -Force -Path $outputParent | Out-Null
    # The visual tool deliberately permits inputs only beneath the run root. Copy the immutable
    # repository config into the retained evidence directory instead of widening that boundary.
    $config = Join-Path $outputParent "exact-v1.config.json"
    Copy-Item -LiteralPath (Join-Path $RepositoryRoot "tools/ui-visual-audit/fixtures/comparison/exact-v1.config.json") -Destination $config
    foreach ($capture in @($Audit.captures)) {
        $attempt = @($capture.attempts | Where-Object { $_.number -eq $capture.selected_attempt })[0]
        if ($null -eq $attempt) {
            throw "Audit capture has no selected screenshot: $($capture.device) / $($capture.state)"
        }
        $screenshot = [string]$attempt.preview.command.screenshot_path
        $outputDirectory = Join-Path $outputParent ("{0}--{1}" -f $capture.device, $capture.state.Replace('.', '_'))
        $log = Join-Path $LogDirectory ("reference-integrity-{0}-{1}.log" -f $capture.device, $capture.state.Replace('.', '_'))
        $command = Invoke-Stage11Command -Name ("reference-integrity-{0}-{1}" -f $capture.device, $capture.state) -Program "cargo" -ArgumentList @(
            "run", "--quiet", "--manifest-path", "tools/ui-visual-audit/Cargo.toml", "--",
            "compare",
            "--repository-root", $RepositoryRoot,
            "--allowed-input-root", $RunRoot,
            "--allowed-output-root", $RunRoot,
            "--reference", $screenshot,
            "--actual", $screenshot,
            "--config", $config,
            "--output-directory", $outputDirectory
        ) -LogPath $log -ExpectJson
        $results.Add($command.json)
    }
    return @($results.ToArray())
}

$scriptRoot = if ([string]::IsNullOrWhiteSpace($PSScriptRoot)) { Split-Path -Parent $PSCommandPath } else { $PSScriptRoot }
$repositoryRoot = Get-Stage11FullPath (Join-Path $scriptRoot "..")
$runnerPowerShell = Get-Command pwsh -ErrorAction SilentlyContinue
if ($null -eq $runnerPowerShell) {
    throw "Stage 11 Runner self-tests require PowerShell 7 (`pwsh`); Windows PowerShell 5.1 cannot parse scripts/run-ui-audit.ps1."
}
Set-Location $repositoryRoot

if ([string]::IsNullOrWhiteSpace($RunId)) {
    $RunId = "stage11-e2e-" + (Get-Date -Format "yyyyMMdd-HHmmss") + "-" + [Guid]::NewGuid().ToString("N").Substring(0, 8)
}
if ($RunId -notmatch "^[A-Za-z0-9][A-Za-z0-9_-]{2,95}$") {
    throw "RunId must be a safe 3..96 character segment."
}

$reportRoot = Join-Path $repositoryRoot (Join-Path "summary/ui-generation" ($RunId + "-report"))
if (Test-Path -LiteralPath $reportRoot) {
    throw "Stage 11 report directory already exists: $reportRoot"
}
$logsRoot = Join-Path $reportRoot "logs"
New-Item -ItemType Directory -Force -Path $logsRoot | Out-Null
$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ($RunId + "-input")
$worktreeBefore = (& git status --porcelain) -join "`n"
$commands = New-Object System.Collections.Generic.List[object]
$started = [System.DateTimeOffset]::UtcNow
$overall = [System.Diagnostics.Stopwatch]::StartNew()

try {
    $regularTask = New-Stage11Task -FixtureTask (Join-Path $repositoryRoot "tools/ui-generation/fixtures/acceptance/task.valid.json") -RunIdValue ($RunId + "-regular") -TemporaryRoot $temporaryRoot -RepositoryRoot $repositoryRoot
    $complexTask = New-Stage11Task -FixtureTask (Join-Path $repositoryRoot "tools/ui-generation/fixtures/acceptance/complex.task.valid.json") -RunIdValue ($RunId + "-complex") -TemporaryRoot $temporaryRoot -RepositoryRoot $repositoryRoot

    $regularGeneration = Invoke-Stage11Command -Name "generate-regular" -Program "cargo" -ArgumentList @(
        "run", "--quiet", "--manifest-path", "tools/ui-generation/Cargo.toml", "--",
        "generate-fixture", "--task", $regularTask, "--repository-root", $repositoryRoot,
        "--document-id", "generated.stage11_regular", "--fixture-profile", "regular"
    ) -LogPath (Join-Path $logsRoot "generate-regular.log") -ExpectJson
    $commands.Add($regularGeneration)

    $complexGeneration = Invoke-Stage11Command -Name "generate-complex" -Program "cargo" -ArgumentList @(
        "run", "--quiet", "--manifest-path", "tools/ui-generation/Cargo.toml", "--",
        "generate-fixture", "--task", $complexTask, "--repository-root", $repositoryRoot,
        "--document-id", "generated.stage11_complex", "--fixture-profile", "complex"
    ) -LogPath (Join-Path $logsRoot "generate-complex.log") -ExpectJson
    $commands.Add($complexGeneration)

    $generatedRuns = @($regularGeneration.json, $complexGeneration.json)
    $documentAudits = New-Object System.Collections.Generic.List[object]
    $referenceChecks = New-Object System.Collections.Generic.List[object]
    foreach ($generatedRun in $generatedRuns) {
        $generatedRunRoot = ConvertFrom-Stage11ExtendedPath ([string]$generatedRun.run_root)
        $generatedDocument = ConvertFrom-Stage11ExtendedPath ([string]$generatedRun.generated_document)
        $auditOutput = Join-Path $generatedRunRoot "desktop-audit"
        $auditName = "audit-" + $generatedRun.fixture_profile
        $audit = Invoke-Stage11Command -Name $auditName -Program "cargo" -ArgumentList @(
            "run", "--quiet", "--manifest-path", "tools/ui-generation/Cargo.toml", "--",
            "audit-document", "--document", $generatedDocument,
            "--output-directory", $auditOutput, "--repository-root", $repositoryRoot,
            "--states", "initial"
        ) -LogPath (Join-Path $logsRoot ($auditName + ".log")) -ExpectJson
        $commands.Add($audit)
        $documentAudits.Add($audit.json)
        foreach ($comparison in Invoke-Stage11IdentityReferenceCheck -Audit $audit.json -RunRoot $generatedRunRoot -RepositoryRoot $repositoryRoot -LogDirectory $logsRoot) {
            $referenceChecks.Add($comparison)
        }
    }

    $stateAudit = Invoke-Stage11Command -Name "audit-multi-state" -Program "cargo" -ArgumentList @(
        "run", "--quiet", "--manifest-path", "tools/ui-generation/Cargo.toml", "--",
        "audit-document",
        "--document", (Join-Path $repositoryRoot "tools/ui-generation/fixtures/audit/phone_tablet_multi_state.valid.json"),
        "--output-directory", (Join-Path $reportRoot "multi-state-audit"),
        "--repository-root", $repositoryRoot,
        "--states", "initial,loading,empty,error,fixture.selected,fixture.disabled,fixture.modal",
        "--require-distinct-from-initial", "loading,empty,error,fixture.selected,fixture.disabled,fixture.modal"
    ) -LogPath (Join-Path $logsRoot "audit-multi-state.log") -ExpectJson
    $commands.Add($stateAudit)

    $operations = Invoke-Stage11Command -Name "operations-stress-fixture" -Program "cargo" -ArgumentList @(
        "run", "--quiet", "--manifest-path", "tools/ui-generation/Cargo.toml", "--", "operations-stress-fixture"
    ) -LogPath (Join-Path $logsRoot "operations-stress-fixture.log") -ExpectJson
    $commands.Add($operations)

    $failureTests = @(
        "repair::tests::provider_unavailable_timeout_and_cancel_are_stable",
        "closed_loop_generation::tests::preview_timeout_is_persisted_as_a_terminal_failed_manifest",
        "promotion::tests::rejected_decision_blocks_planning_without_writing_formal_files"
    )
    foreach ($testName in $failureTests) {
        $safeName = $testName.Replace(":", "_")
        $test = Invoke-Stage11Command -Name ("failure-" + $safeName) -Program "cargo" -ArgumentList @(
            "test", "--manifest-path", "tools/ui-generation/Cargo.toml", $testName, "--", "--exact"
        ) -LogPath (Join-Path $logsRoot ("failure-" + $safeName + ".log"))
        $commands.Add($test)
    }

    $worktreeSelfTest = Invoke-Stage11Command -Name "runner-worktree-isolation-self-test" -Program $runnerPowerShell.Source -ArgumentList @(
        "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "scripts/run-ui-audit.ps1", "-SelfTestWorktree"
    ) -LogPath (Join-Path $logsRoot "runner-worktree-isolation-self-test.log")
    $commands.Add($worktreeSelfTest)

    if (-not $SkipRunnerSelfTest) {
        $runnerSelfTest = Invoke-Stage11Command -Name "runner-self-test" -Program $runnerPowerShell.Source -ArgumentList @(
            "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "scripts/run-ui-audit.ps1", "-SelfTest"
        ) -LogPath (Join-Path $logsRoot "runner-self-test.log")
        $commands.Add($runnerSelfTest)
    } elseif (-not [string]::IsNullOrWhiteSpace($RunnerSelfTestReceipt)) {
        $receiptPath = Get-Stage11FullPath $RunnerSelfTestReceipt
        if (-not (Test-Path -LiteralPath $receiptPath)) {
            throw "Runner self-test receipt was not found: $receiptPath"
        }
        $receipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
        if ([string]$receipt.name -ne "runner-self-test" -or [int]$receipt.exit_code -ne 0) {
            throw "Runner self-test receipt is not a passed runner-self-test command: $receiptPath"
        }
        $commands.Add($receipt)
    }

    $desktopRunner = $null
    if (-not [string]::IsNullOrWhiteSpace($ExistingDesktopManifest)) {
        $desktopManifestPath = Get-Stage11FullPath $ExistingDesktopManifest
        if (-not (Test-Path -LiteralPath $desktopManifestPath)) {
            throw "Existing desktop Runner manifest was not found: $desktopManifestPath"
        }
        $desktopManifest = Get-Content -LiteralPath $desktopManifestPath -Raw | ConvertFrom-Json
        if ([string]$desktopManifest.status -ne "passed" -or [int]$desktopManifest.summary.failed -ne 0) {
            throw "Existing desktop Runner manifest is not a passed matrix: $desktopManifestPath"
        }
        $desktopRunner = [pscustomobject]@{
            name = "desktop-ui-gallery-reused"
            source_manifest = $desktopManifestPath
            status = "passed"
        }
        $commands.Add($desktopRunner)
    } elseif (-not $SkipDesktopRunner) {
        $desktopRunId = $RunId + "-desktop"
        $desktopRunner = Invoke-Stage11Command -Name "desktop-ui-gallery" -Program $runnerPowerShell.Source -ArgumentList @(
            "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "scripts/run-ui-audit.ps1",
            "-RunId", $desktopRunId,
            "-Screens", "ui-gallery",
            "-Devices", "phone-small,phone-portrait,tablet-portrait,tablet-landscape",
            "-States", "visual_acceptance,middle,bottom",
            "-DeterministicCapture", "-RepeatCaptures", "2", "-AnalysisMode", "Fixture",
            "-AnalysisResultPath", "tools/ui-visual-audit/fixtures/ai/fixture-response.json"
        ) -LogPath (Join-Path $logsRoot "desktop-ui-gallery.log")
        $commands.Add($desktopRunner)
        $desktopManifestPath = Join-Path $repositoryRoot (Join-Path "summary/ui-audit" ($desktopRunId + "/manifest.json"))
        if (-not (Test-Path -LiteralPath $desktopManifestPath)) {
            throw "Desktop Runner did not create a manifest: $desktopManifestPath"
        }
        $desktopManifest = Get-Content -LiteralPath $desktopManifestPath -Raw | ConvertFrom-Json
    } else {
        $desktopManifest = $null
    }

    $adb = Get-Command adb -ErrorAction SilentlyContinue
    $androidBlocker = if ($null -eq $adb) {
        "No adb executable is available on PATH, and the Runner has no validated remote Android screenshot plus system metadata contract."
    } else {
        "adb is present but was deliberately not queried. The Runner still has no validated remote Android screenshot plus system metadata contract, so this offline run cannot claim real-device evidence."
    }
    $android = [ordered]@{
        status = "external_blocked"
        adb_available = ($null -ne $adb)
        remote_execution_attempted = $false
        adminapi_attempted = $false
        credential_or_provider_used = $false
        blocker = $androidBlocker
        required_evidence = @(
            "authorized API 31+ Android device with a built arm64 Debug APK",
            "remote screenshot and metadata contract for safe area, IME, touch, density, orientation, fonts, nine-slice, and material fallback",
            "explicitly authorized Remote Http execution"
        )
    }

    $worktreeAfter = (& git status --porcelain) -join "`n"
    if ($worktreeAfter -ne $worktreeBefore) {
        throw "The acceptance commands changed the caller worktree. See git status before accepting this run."
    }

    $overall.Stop()
    $report = [ordered]@{
        schema_version = 1
        stage = "11"
        run_id = $RunId
        status = "passed_with_external_android_blocker"
        started_utc = $started.ToString("o")
        elapsed_ms = [int64]$overall.ElapsedMilliseconds
        repository_root = $repositoryRoot
        scope = [ordered]@{
            offline_only = $true
            provider_calls = "repository fixture provider only"
            remote_execution_attempted = $false
            caller_worktree_unchanged = $true
        }
        generated_runs = $generatedRuns
        document_audits = @($documentAudits.ToArray())
        multi_state_audit = $stateAudit.json
        reference_integrity_checks = @($referenceChecks.ToArray())
        desktop_runner_manifest = $desktopManifest
        operations_stress = $operations.json
        metrics = [ordered]@{
            generation_elapsed_ms = @($commands | Where-Object { $_.name -in @("generate-regular", "generate-complex") } | ForEach-Object { $_.elapsed_ms })
            model_cost_microunits = @($generatedRuns | ForEach-Object { (Get-Content -LiteralPath (ConvertFrom-Stage11ExtendedPath ([string]$_.run_report)) -Raw | ConvertFrom-Json).estimated_cost_micro_units })
            iteration_count = "repair traces and runner fixture iterations are retained in generated run artifacts and command logs"
            peak_memory = "not instrumented by the standalone preview; visual budget metadata is retained by the desktop Runner when it is enabled"
            screenshot_stability = if ($null -eq $desktopManifest) { "not run" } else { "deterministic Runner used RepeatCaptures=2; inspect manifest captures for per-state evidence" }
            visual_audit = "all standalone document audits and exact self-reference integrity checks passed"
        }
        failure_rehearsals = [ordered]@{
            provider_timeout_and_no_network = "offline unit fixture passed; no network call was made"
            device_offline = "external blocker recorded without a remote/adminapi call"
            compilation_failure = "Runner SelfTest fixture exercises command check failure when enabled"
            visual_degradation = "Runner SelfTest mock degradation fixture exercises regression stop when enabled"
            human_reject_promotion = "promotion rejection unit fixture passed"
            failed_run_recovery = "preview failure manifest and worktree isolation fixtures passed"
        }
        android = $android
        commands = @($commands.ToArray())
        cleanup = [ordered]@{
            temporary_input_root = $temporaryRoot
            temporary_input_cleanup = "The finally block removes this temporary input root on every script exit."
            retained_evidence = @($reportRoot) + @($generatedRuns | ForEach-Object { $_.run_root })
        }
    }
    Write-Stage11Json -Path (Join-Path $reportRoot "acceptance-report.json") -Value $report

    $markdown = @"
# Stage 11 Offline E2E Acceptance

- Run: ``$RunId``
- Status: ``passed_with_external_android_blocker``
- Total elapsed milliseconds: $($report.elapsed_ms)
- Offline fixture runs: regular and complex profiles both completed generation, validation/repair, preview, four-profile audit, and identity reference-integrity comparison.
- Multi-state audit: initial, loading, empty, error, selected, disabled, and modal were captured on all four desktop profiles.
- Desktop Runner: $(if ($null -eq $desktopManifest) { "skipped" } else { "ui_gallery deterministic captures with repeat count 2" }).
- Android: blocked externally. No provider, remote device, adminapi, or credential was used. $($android.blocker)

The JSON report contains command logs, retained artifact paths, fixture-only cost values, and the precise remaining Android evidence required.
"@
    Set-Content -LiteralPath (Join-Path $reportRoot "acceptance-report.md") -Value $markdown -Encoding UTF8
} finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}
