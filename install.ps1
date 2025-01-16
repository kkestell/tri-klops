$ErrorActionPreference = 'Stop'

cargo build --release

if ($LASTEXITCODE -ne 0) {
    Write-Error "cargo build failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" install.iss
