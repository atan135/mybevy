$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Resolve-Path (Join-Path $scriptRoot "..\project")
$defaultGameArguments = @(
    "--window-size"
    "2772x1280"
    "--device-scale"
    "3.25"
    "--window-scale"
    "50%"
)
$gameArguments = @($defaultGameArguments) + @($args)
$cargoArguments = @("run", "--locked", "--profile", "dev-fast")

if ($env:OS -eq "Windows_NT") {
    Write-Host "run_fast platform=windows mode=static-dev-fast reason=bevy_dylib_windows_link_limit"
}
else {
    $cargoArguments += @("--features", "bevy/dynamic_linking")
}
$cargoArguments += @("--") + $gameArguments

Push-Location $projectRoot
try {
    & cargo @cargoArguments
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}

exit $exitCode
