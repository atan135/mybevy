param(
    [string]$HostPlayerId = "host-a",
    [string]$ClientPlayerId = "client-b",
    [string]$HostAddress = "127.0.0.1",
    [int]$Port = 15000,
    [ValidateSet("tcp", "kcp")]
    [string]$Transport = "tcp",
    [string]$StartScreen = "touch",
    [int]$HostStartupDelayMs = 1500,
    [switch]$SkipBuild,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptRoot "..")
$projectRoot = Join-Path $repoRoot "project"
$logRoot = Join-Path $repoRoot "logs\two-clients"
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
    throw "Client binary not found: $clientBinary. Run without -SkipBuild first."
}

function ConvertTo-PowerShellLiteral {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    "'$($Value -replace "'", "''")'"
}

function Start-Client {
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

    $envLines = $Environment.GetEnumerator() | ForEach-Object {
        '$env:{0}={1}' -f $_.Key, (ConvertTo-PowerShellLiteral ([string]$_.Value))
    }

    $launcherPath = Join-Path $launchRoot "$Name.ps1"
    $cmdCommand = '"{0}" 2>&1' -f $clientBinary
    $launcherLines = @(
        '$ErrorActionPreference = "Stop"',
        ('Set-Location {0}' -f (ConvertTo-PowerShellLiteral $projectRoot)),
        $envLines
    ) + @(
        ('& cmd.exe /d /c {0} | Tee-Object -FilePath {1}' -f (ConvertTo-PowerShellLiteral $cmdCommand), (ConvertTo-PowerShellLiteral $LogFile))
    )

    Set-Content -Path $launcherPath -Value $launcherLines -Encoding UTF8

    if ($DryRun) {
        Write-Host "Prepared $Title"
        Write-Host "  log: $LogFile"
        Write-Host "  launcher: $launcherPath"
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
    TOUCH_START_SCREEN = $StartScreen
    TOUCH_AUTO_LOCAL_AUTHORITY = "false"
    AUTHORITY_TRANSPORT = $Transport
    AUTHORITY_DEV_AUTO_INPUT = "false"
    RUST_LOG = "info"
}

$hostEnv = $commonEnv.Clone()
$hostEnv["AUTHORITY_DEV_MODE"] = "lan-host"
$hostEnv["AUTHORITY_PLAYER_ID"] = $HostPlayerId
$hostEnv["AUTHORITY_BIND_ADDR"] = $bindAddr
$hostEnv["TOUCH_PLAYER_ID"] = $HostPlayerId

$clientEnv = $commonEnv.Clone()
$clientEnv["AUTHORITY_REMOTE_HOST"] = $HostAddress
$clientEnv["AUTHORITY_REMOTE_PORT"] = "$Port"
$clientEnv["TOUCH_AUTHORITY_MODE"] = "lan-client"
$clientEnv["TOUCH_PLAYER_ID"] = $ClientPlayerId

Start-Client `
    -Title "host client ($HostPlayerId)" `
    -Name "host" `
    -Environment $hostEnv `
    -LogFile (Join-Path $logRoot "host.log")

if (-not $DryRun) {
    Start-Sleep -Milliseconds $HostStartupDelayMs
}

Start-Client `
    -Title "joining client ($ClientPlayerId)" `
    -Name "client" `
    -Environment $clientEnv `
    -LogFile (Join-Path $logRoot "client.log")

Write-Host ""
if ($DryRun) {
    Write-Host "Dry run complete. No clients were started."
} else {
    Write-Host "Two clients are starting."
}
Write-Host "Host endpoint: $bindAddr ($Transport)"
