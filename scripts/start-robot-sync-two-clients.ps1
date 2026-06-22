param(
    [ValidateSet("myserver", "lan")]
    [string]$Mode = "myserver",
    [string]$RoomId = "robot-sync-room",
    [string]$PolicyId = "robot_sync_room",
    [string]$PlayerAId = "robot-player-a",
    [string]$PlayerBId = "robot-player-b",
    [string]$GuestAId = "robot-guest-a",
    [string]$GuestBId = "robot-guest-b",
    [ValidateSet("bot", "manual", "off")]
    [string]$InputModeA = "bot",
    [ValidateSet("bot", "manual", "off")]
    [string]$InputModeB = "bot",
    [ValidateSet("tcp", "kcp")]
    [string]$Transport = "tcp",
    [string]$HostAddress = "127.0.0.1",
    [int]$Port = 15000,
    [int]$StartupDelayMs = 1500,
    [switch]$SkipBuild,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptRoot "..")
$projectRoot = Join-Path $repoRoot "project"
$runId = Get-Date -Format "yyyyMMdd-HHmmss-fff"
$logRoot = Join-Path $repoRoot (Join-Path "logs" (Join-Path "robot-sync-two-clients" $runId))
$launchRoot = Join-Path $logRoot "launch"

New-Item -ItemType Directory -Force -Path $logRoot | Out-Null
New-Item -ItemType Directory -Force -Path $launchRoot | Out-Null

if (-not $SkipBuild) {
    Push-Location $projectRoot
    try {
        cargo build
    }
    finally {
        Pop-Location
    }
}

$binaryName = if ([System.IO.Path]::DirectorySeparatorChar -eq '\') { "project.exe" } else { "project" }
$clientBinary = Join-Path $projectRoot (Join-Path "target" (Join-Path "debug" $binaryName))
if (-not (Test-Path $clientBinary)) {
    if ($DryRun) {
        Write-Warning "Client binary not found: $clientBinary. Dry run will continue; run without -SkipBuild before starting clients."
    } else {
        throw "Client binary not found: $clientBinary. Run without -SkipBuild first."
    }
}

function ConvertTo-PowerShellLiteral {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    "'$($Value -replace "'", "''")'"
}

function Write-DryRunClient {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Title,
        [Parameter(Mandatory = $true)]
        [hashtable]$Environment,
        [Parameter(Mandatory = $true)]
        [string]$LogFile,
        [Parameter(Mandatory = $true)]
        [string]$LauncherPath
    )

    Write-Host "Prepared $Title"
    Write-Host "  log: $LogFile"
    Write-Host "  launcher: $LauncherPath"
    Write-Host "  environment:"
    $Environment.GetEnumerator() |
        Sort-Object Key |
        ForEach-Object {
            Write-Host ("    {0}={1}" -f $_.Key, $_.Value)
        }
}

function Start-RobotSyncClient {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Title,
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [hashtable]$Environment,
        [Parameter(Mandatory = $true)]
        [string]$LogFile
    )

    $envLines = $Environment.GetEnumerator() |
        Sort-Object Key |
        ForEach-Object {
            '$env:{0}={1}' -f $_.Key, (ConvertTo-PowerShellLiteral ([string]$_.Value))
        }

    $launcherPath = Join-Path $launchRoot "$Name.ps1"
    $cmdCommand = '"{0}" 2>&1' -f $clientBinary
    $launcherLines = @(
        '$ErrorActionPreference = "Stop"',
        ('Set-Location {0}' -f (ConvertTo-PowerShellLiteral $projectRoot)),
        '$env:AUTHORITY_DEV_MODE=$null',
        $envLines
    ) + @(
        ('& cmd.exe /d /c {0} | Tee-Object -FilePath {1}' -f (ConvertTo-PowerShellLiteral $cmdCommand), (ConvertTo-PowerShellLiteral $LogFile))
    )

    Set-Content -Path $launcherPath -Value $launcherLines -Encoding UTF8

    if ($DryRun) {
        Write-DryRunClient `
            -Title $Title `
            -Environment $Environment `
            -LogFile $LogFile `
            -LauncherPath $launcherPath
        return
    }

    Start-Process powershell -ArgumentList @(
        "-NoExit",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        $launcherPath
    ) -WorkingDirectory $projectRoot -WindowStyle Normal | Out-Null

    Write-Host "Started $Title"
    Write-Host "  log: $LogFile"
    Write-Host "  launcher: $launcherPath"
}

$bindAddr = "${HostAddress}:$Port"
$commonEnv = @{
    MYBEVY_START_SCENE = "arena.robot_sync"
    AUTHORITY_DEV_AUTO_INPUT = "false"
    AUTHORITY_TRANSPORT = $Transport
    ROBOT_SYNC_TRANSPORT = $Transport
    RUST_LOG = "info"
}

if ($Mode -eq "myserver") {
    $myserverPortEnvName = if ($Transport -eq "kcp") { "MYSERVER_KCP_PORT" } else { "MYSERVER_TCP_FALLBACK_PORT" }

    $clientAEnv = $commonEnv.Clone()
    $clientAEnv["ROBOT_SYNC_AUTHORITY_MODE"] = "myserver"
    $clientAEnv["AUTHORITY_PLAYER_ID"] = $PlayerAId
    $clientAEnv["ROBOT_SYNC_PLAYER_ID"] = $PlayerAId
    $clientAEnv["ROBOT_SYNC_INPUT_MODE"] = $InputModeA
    $clientAEnv["AUTHORITY_MYSERVER_ROOM"] = $RoomId
    $clientAEnv["ROBOT_SYNC_MYSERVER_ROOM"] = $RoomId
    $clientAEnv["AUTHORITY_MYSERVER_POLICY"] = $PolicyId
    $clientAEnv["ROBOT_SYNC_MYSERVER_POLICY"] = $PolicyId
    $clientAEnv["AUTHORITY_MYSERVER_GUEST_ID"] = $GuestAId
    $clientAEnv["ROBOT_SYNC_MYSERVER_GUEST_ID"] = $GuestAId
    $clientAEnv["MYSERVER_GUEST_ID"] = $GuestAId
    $clientAEnv["MYSERVER_TRANSPORT"] = $Transport
    $clientAEnv["MYSERVER_GAME_HOST"] = $HostAddress
    $clientAEnv[$myserverPortEnvName] = "$Port"

    $clientBEnv = $commonEnv.Clone()
    $clientBEnv["ROBOT_SYNC_AUTHORITY_MODE"] = "myserver"
    $clientBEnv["AUTHORITY_PLAYER_ID"] = $PlayerBId
    $clientBEnv["ROBOT_SYNC_PLAYER_ID"] = $PlayerBId
    $clientBEnv["ROBOT_SYNC_INPUT_MODE"] = $InputModeB
    $clientBEnv["AUTHORITY_MYSERVER_ROOM"] = $RoomId
    $clientBEnv["ROBOT_SYNC_MYSERVER_ROOM"] = $RoomId
    $clientBEnv["AUTHORITY_MYSERVER_POLICY"] = $PolicyId
    $clientBEnv["ROBOT_SYNC_MYSERVER_POLICY"] = $PolicyId
    $clientBEnv["AUTHORITY_MYSERVER_GUEST_ID"] = $GuestBId
    $clientBEnv["ROBOT_SYNC_MYSERVER_GUEST_ID"] = $GuestBId
    $clientBEnv["MYSERVER_GUEST_ID"] = $GuestBId
    $clientBEnv["MYSERVER_TRANSPORT"] = $Transport
    $clientBEnv["MYSERVER_GAME_HOST"] = $HostAddress
    $clientBEnv[$myserverPortEnvName] = "$Port"
} else {
    $clientAEnv = $commonEnv.Clone()
    $clientAEnv["ROBOT_SYNC_AUTHORITY_MODE"] = "lan-host"
    $clientAEnv["AUTHORITY_PLAYER_ID"] = $PlayerAId
    $clientAEnv["ROBOT_SYNC_PLAYER_ID"] = $PlayerAId
    $clientAEnv["ROBOT_SYNC_INPUT_MODE"] = $InputModeA
    $clientAEnv["AUTHORITY_BIND_ADDR"] = $bindAddr
    $clientAEnv["ROBOT_SYNC_LAN_BIND_ADDR"] = $bindAddr

    $clientBEnv = $commonEnv.Clone()
    $clientBEnv["ROBOT_SYNC_AUTHORITY_MODE"] = "lan-client"
    $clientBEnv["AUTHORITY_PLAYER_ID"] = $PlayerBId
    $clientBEnv["ROBOT_SYNC_PLAYER_ID"] = $PlayerBId
    $clientBEnv["ROBOT_SYNC_INPUT_MODE"] = $InputModeB
    $clientBEnv["AUTHORITY_REMOTE_HOST"] = $HostAddress
    $clientBEnv["AUTHORITY_REMOTE_PORT"] = "$Port"
    $clientBEnv["ROBOT_SYNC_REMOTE_HOST"] = $HostAddress
    $clientBEnv["ROBOT_SYNC_REMOTE_PORT"] = "$Port"
}

Start-RobotSyncClient `
    -Title "robot sync client A ($PlayerAId)" `
    -Name "client-a" `
    -Environment $clientAEnv `
    -LogFile (Join-Path $logRoot "client-a.log")

if (-not $DryRun) {
    Start-Sleep -Milliseconds $StartupDelayMs
}

Start-RobotSyncClient `
    -Title "robot sync client B ($PlayerBId)" `
    -Name "client-b" `
    -Environment $clientBEnv `
    -LogFile (Join-Path $logRoot "client-b.log")

Write-Host ""
if ($DryRun) {
    Write-Host "Dry run complete. No clients were started."
} else {
    Write-Host "Robot sync clients are starting."
}

Write-Host "Mode: $Mode"
Write-Host "Input modes: A=$InputModeA, B=$InputModeB"
Write-Host "Transport: $Transport"
if ($Mode -eq "myserver") {
    Write-Host "Room: $RoomId"
    Write-Host "Policy: $PolicyId"
    Write-Host "Guests: $GuestAId, $GuestBId"
} else {
    Write-Host "LAN endpoint: $bindAddr"
}
Write-Host "Logs: $logRoot"
