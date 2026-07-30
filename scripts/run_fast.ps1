$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Resolve-Path (Join-Path $scriptRoot "..\project")
$gameArguments = @($args)

Push-Location $projectRoot
try {
    & cargo run --locked --features bevy/dynamic_linking -- @gameArguments
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}

exit $exitCode
