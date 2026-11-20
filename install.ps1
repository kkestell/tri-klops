$ErrorActionPreference = 'Stop'

Push-Location $PSScriptRoot
try {
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    & "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" install.iss
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
