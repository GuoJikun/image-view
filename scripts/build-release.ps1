#Requires -Version 5.1
<#
.SYNOPSIS
    Build the release executable and the NSIS installer with cargo-packager.

.DESCRIPTION
    Reads the version from Cargo.toml, runs `cargo build --release --locked`,
    packages the installer with `cargo packager --release`, and writes
    dist\SHA256SUMS.txt plus dist\update.json (the auto-update manifest).

    cargo-packager re-runs an incremental release build through
    `beforePackagingCommand`, so the explicit build below is mostly a size and
    failure check; it costs well under a second once the artifacts are fresh.

    The NSIS toolchain must already exist in %LOCALAPPDATA%\.cargo-packager.
    cargo-packager deletes and re-downloads that entire directory when any
    required file is missing, which fails on restricted networks.

.PARAMETER SkipBuild
    Skip the explicit `cargo build --release --locked` step.

.PARAMETER SkipInstaller
    Only build the executable (no packaging step).

.PARAMETER Notes
    Release notes written to update.json. Defaults to the matching section of
    CHANGELOG.md.

.PARAMETER DownloadBaseUrl
    Base URL for the installer. The default points at the latest GitHub Release.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
#>
[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$SkipInstaller,
    [string]$Notes,
    [string]$DownloadBaseUrl = 'https://github.com/GuoJikun/image-view/releases/latest/download'
)

# Extract the "## [version]" section from CHANGELOG.md for update.json notes.
function Get-ChangelogSection {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Version
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return ''
    }

    $lines = @(Get-Content -LiteralPath $Path -Encoding UTF8)
    $heading = '^\s*##\s+\[' + [regex]::Escape($Version) + '\]'
    $start = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match $heading) {
            $start = $i + 1
            break
        }
    }
    if ($start -lt 0) {
        return ''
    }

    $body = New-Object System.Collections.Generic.List[string]
    for ($i = $start; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^\s*##\s+') {
            break
        }
        $body.Add($lines[$i])
    }
    return ($body -join "`n").Trim()
}

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

    if (-not (Get-Command cargo-packager -ErrorAction SilentlyContinue)) {
        throw 'cargo-packager not found. Install it with: cargo install cargo-packager --locked'
    }

    $nsis = Join-Path $env:LOCALAPPDATA '.cargo-packager\NSIS\Bin\makensis.exe'
    if (-not (Test-Path -LiteralPath $nsis)) {
        throw "NSIS toolchain missing at $nsis. cargo-packager would try to download it; provision the files first (see docs\NSIS-TOOLCHAIN.md)."
    }

    $dist = Join-Path $root 'dist'
    New-Item -ItemType Directory -Force -Path $dist | Out-Null

    Write-Host 'Packaging with cargo-packager...'
    & cargo packager --release -c (Join-Path $root 'Packager.toml')
    if ($LASTEXITCODE -ne 0) {
        throw "cargo packager failed with exit code $LASTEXITCODE."
    }

    # cargo-packager names the artifact image-view_<version>_<arch>-setup.exe.
    $setup = Get-ChildItem -LiteralPath $dist -Filter "image-view_${version}_*-setup.exe" |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if (-not $setup) {
        throw "Missing installer output: $dist\image-view_${version}_<arch>-setup.exe"
    }

    $hash = (Get-FileHash -LiteralPath $setup.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $sums = "$hash  $($setup.Name)"
    Set-Content -LiteralPath (Join-Path $dist 'SHA256SUMS.txt') -Value $sums -Encoding ascii

    if (-not $Notes) {
        $Notes = Get-ChangelogSection -Path (Join-Path $root 'CHANGELOG.md') -Version $version
    }
    $manifest = [ordered]@{
        version = $version
        url     = '{0}/{1}' -f $DownloadBaseUrl.TrimEnd('/'), $setup.Name
        sha256  = $hash
        notes   = $Notes
    }
    # serde_json rejects a UTF-8 BOM, so write without one (PowerShell 5.1 adds it with -Encoding utf8).
    $manifestPath = Join-Path $dist 'update.json'
    $manifestJson = $manifest | ConvertTo-Json -Depth 3
    [System.IO.File]::WriteAllText($manifestPath, $manifestJson, (New-Object System.Text.UTF8Encoding($false)))

    Write-Host ("Installer: {0} ({1:N0} bytes)" -f $setup.FullName, $setup.Length)
    Write-Host ("SHA256:    {0}" -f $hash)
    Write-Host ("Manifest:  {0}" -f $manifestPath)
    Write-Host 'Done.'
}
finally {
    Pop-Location
}
