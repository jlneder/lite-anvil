# Build a local Windows x86_64 release artifact matching the GitHub Actions release output.
# Produces:
#   dist\lite-anvil-${Version}-windows-x86_64\        (staging directory)
#   dist\lite-anvil-${Version}-windows-x86_64.zip     (release archive)
#Requires -Version 5.1
$ErrorActionPreference = 'Stop'

$RootDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $RootDir

$CargoToml = Join-Path $RootDir 'Cargo.toml'
$Version = ''
if (Test-Path $CargoToml) {
    $inPackage = $false
    foreach ($line in Get-Content $CargoToml) {
        if ($line -match '^\[package\]') { $inPackage = $true; continue }
        if ($line -match '^\[') { $inPackage = $false }
        if ($inPackage -and $line -match '^version = "([^"]+)"$') {
            $Version = $Matches[1]
            break
        }
    }
}
if (-not $Version) {
    Write-Error "Could not read version from Cargo.toml"
    exit 1
}

$ArchiveBase = "lite-anvil-$Version-windows-x86_64"
$DistDir = Join-Path $RootDir 'dist'
$StageDir = Join-Path $DistDir $ArchiveBase
$Archive = Join-Path $DistDir "$ArchiveBase.zip"

cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$Binary = Join-Path $RootDir 'target\release\lite-anvil.exe'
if (-not (Test-Path $Binary)) {
    Write-Error "Binary not found at $Binary"
    exit 1
}

if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir }
if (Test-Path $Archive)  { Remove-Item -Force $Archive }
New-Item -ItemType Directory -Force -Path $StageDir | Out-Null

Copy-Item -Path $Binary -Destination $StageDir
Copy-Item -Path (Join-Path $RootDir 'data') -Destination $StageDir -Recurse
$WindowsResources = Join-Path $RootDir 'resources\windows\*.ps1'
if (Test-Path $WindowsResources) {
    Copy-Item -Path $WindowsResources -Destination $StageDir
}

Compress-Archive -Path $StageDir -DestinationPath $Archive

Write-Host "Built archive: $Archive"
Write-Host "Staging dir:   $StageDir"
