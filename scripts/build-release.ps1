#Requires -Version 5.1
<#
.SYNOPSIS
    Build the release executable and the Inno Setup installer.

.DESCRIPTION
    Reads the version from Cargo.toml, runs `cargo build --release --locked`,
    compiles installer\image-view.iss with ISCC, and writes dist\SHA256SUMS.txt.

.PARAMETER SkipBuild
    Reuse an existing target\release\image-view.exe.

.PARAMETER SkipInstaller
    Only build the executable (no ISCC step).

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
#>
[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$SkipInstaller
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root

try {
    $cargoToml = Get-Content -LiteralPath (Join-Path $root 'Cargo.toml') -Raw -Encoding UTF8
    if ($cargoToml -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
        throw 'Cannot read the package version from Cargo.toml.'
    }
    $version = $Matches[1]

    $numeric = ($version -split '[-+]')[0]
    if ($numeric -notmatch '^\d+\.\d+\.\d+$') {
        throw "Unsupported version '$version': expected MAJOR.MINOR.PATCH with an optional suffix."
    }
    $fileVersion = "$numeric.0"

    if (-not $SkipBuild) {
        Write-Host "Building image-view $version..."
        & cargo build --release --locked
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed with exit code $LASTEXITCODE."
        }
    }

    $exe = Join-Path $root 'target\release\image-view.exe'
    if (-not (Test-Path -LiteralPath $exe)) {
        throw "Missing release executable: $exe"
    }
    $exeSize = (Get-Item -LiteralPath $exe).Length
    Write-Host ("Executable: {0} ({1:N0} bytes)" -f $exe, $exeSize)

    if ($SkipInstaller) {
        Write-Host 'Skipped installer.'
        return
    }

    $iscc = $env:ISCC_PATH
    if (-not $iscc) {
        $command = Get-Command iscc.exe -ErrorAction SilentlyContinue
        if ($command) {
            $iscc = $command.Source
        }
    }
    if (-not $iscc) {
        $candidates = @()
        foreach ($base in @(${env:ProgramFiles(x86)}, $env:ProgramFiles, $env:LOCALAPPDATA)) {
            if ($base) {
                $candidates += (Join-Path $base 'Inno Setup 6\ISCC.exe')
            }
        }
        if ($env:USERPROFILE) {
            $candidates += (Join-Path $env:USERPROFILE 'scoop\apps\inno-setup\current\ISCC.exe')
        }
        $candidates += 'D:\Scoop\Scoop\apps\inno-setup\current\ISCC.exe'
        foreach ($candidate in $candidates) {
            if (Test-Path -LiteralPath $candidate) {
                $iscc = $candidate
                break
            }
        }
    }
    if (-not $iscc) {
        throw 'ISCC.exe not found. Install Inno Setup 6 or set ISCC_PATH.'
    }

    $dist = Join-Path $root 'dist'
    New-Item -ItemType Directory -Force -Path $dist | Out-Null

    Write-Host "Compiling installer with $iscc..."
    & $iscc "/DAppVersion=$version" "/DAppVersionNumeric=$fileVersion" (Join-Path $root 'installer\image-view.iss')
    if ($LASTEXITCODE -ne 0) {
        throw "ISCC failed with exit code $LASTEXITCODE."
    }

    $setupName = "image-view-$version-setup.exe"
    $setup = Join-Path $dist $setupName
    if (-not (Test-Path -LiteralPath $setup)) {
        throw "Missing installer output: $setup"
    }

    $hash = (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant()
    $setupSize = (Get-Item -LiteralPath $setup).Length
    $sums = "$hash  $setupName"
    Set-Content -LiteralPath (Join-Path $dist 'SHA256SUMS.txt') -Value $sums -Encoding ascii

    Write-Host ("Installer: {0} ({1:N0} bytes)" -f $setup, $setupSize)
    Write-Host ("SHA256:    {0}" -f $hash)
    Write-Host 'Done.'
}
finally {
    Pop-Location
}
