# install.ps1 — Download and install the wasm(1) CLI tool on Windows.
#
# Usage:
#   irm https://github.com/yoshuawuyts/wasm.directory/releases/latest/download/install.ps1 | iex
#
# Environment variables:
#   WASM_VERSION   Install a specific version (e.g. 0.3.0)
#   CARGO_HOME     Override install directory (default: $HOME\.cargo)

param(
    [string]$Version
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Repo = 'yoshuawuyts/wasm.directory'
$BinaryName = 'component'
$Target = 'x86_64-pc-windows-msvc'

# --- resolve version --------------------------------------------------------

function Resolve-LatestVersion {
    $url = "https://github.com/$Repo/releases/latest"
    try {
        # Invoke-WebRequest follows redirects; the final URI contains the tag
        $response = Invoke-WebRequest -Uri $url -MaximumRedirection 10 -ErrorAction Stop -UseBasicParsing
        $finalUrl = $response.BaseResponse.ResponseUri
        if (-not $finalUrl) {
            # PowerShell 7+
            $finalUrl = $response.BaseResponse.RequestMessage.RequestUri
        }
        $tag = ($finalUrl -split '/')[-1]
        return $tag -replace '^v', ''
    } catch {
        throw "Could not resolve latest version from GitHub: $_"
    }
}

# --- main -------------------------------------------------------------------

# Determine version
if (-not $Version) {
    $Version = $env:WASM_VERSION
}
if (-not $Version -or $Version -eq 'latest') {
    $Version = Resolve-LatestVersion
}

$InstallDir = if ($env:CARGO_HOME) { Join-Path $env:CARGO_HOME 'bin' } else { Join-Path $HOME '.cargo\bin' }
$ArchiveUrl = "https://github.com/$Repo/releases/download/$Version/$BinaryName-$Target.zip"
$BinaryPath = Join-Path $InstallDir "$BinaryName.exe"

Write-Host "Installing $BinaryName v$Version ($Target)"
Write-Host "  from: $ArchiveUrl"
Write-Host "  to:   $BinaryPath"
Write-Host ''

# Create a temporary directory
$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

try {
    $ArchivePath = Join-Path $TmpDir 'archive.zip'

    Write-Host 'Downloading...'
    Invoke-WebRequest -Uri $ArchiveUrl -OutFile $ArchivePath -UseBasicParsing

    Write-Host 'Extracting...'
    Expand-Archive -Path $ArchivePath -DestinationPath $TmpDir -Force

    # Install the binary
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    Copy-Item -Path (Join-Path $TmpDir "$BinaryName.exe") -Destination $BinaryPath -Force

    Write-Host ''
    Write-Host "Installed $BinaryName to $BinaryPath"

    # Check if install dir is on PATH
    $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($UserPath -notlike "*$InstallDir*") {
        Write-Host ''
        Write-Host "warning: $InstallDir is not in your PATH"
        Write-Host ''

        # Add to user PATH (avoid duplicates by checking before appending)
        if ($UserPath) {
            $NewPath = "$UserPath;$InstallDir"
        } else {
            $NewPath = $InstallDir
        }
        [Environment]::SetEnvironmentVariable('Path', $NewPath, 'User')
        $env:Path = "$env:Path;$InstallDir"

        Write-Host "Added $InstallDir to your user PATH."
        Write-Host 'Restart your terminal for the change to take effect.'
    } else {
        Write-Host ''
        Write-Host "Run '$BinaryName --version' to verify the installation."
    }
} finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}
