param([string] $ReleaseTag)

$ErrorActionPreference = 'Stop'

Push-Location $PSScriptRoot
try {
    $metadata = cargo metadata --locked --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $version = ($metadata | ConvertFrom-Json).packages[0].version
    if ($ReleaseTag -and $ReleaseTag -cne "v$version") {
        throw "Tag $ReleaseTag does not match the Cargo version (v$version)."
    }

    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    & "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" "/DAppVersion=$version" install.iss
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
