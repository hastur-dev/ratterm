# Ratterm Installer for Windows
# Usage: irm https://raw.githubusercontent.com/OWNER/ratterm/main/install.ps1 | iex
# Or: .\install.ps1 [-Uninstall] [-User]

param(
    [switch]$Uninstall,
    [switch]$User
)

$ErrorActionPreference = "Stop"
$Version = "0.1.0"
$Repo = "hastur-dev/ratterm"
$BinaryName = "rat"
$AppName = "ratterm"

# ASCII art banner
Write-Host ""
Write-Host "  ╦═╗╔═╗╔╦╗╔╦╗╔═╗╦═╗╔╦╗" -ForegroundColor Cyan
Write-Host "  ╠╦╝╠═╣ ║  ║ ║╣ ╠╦╝║║║" -ForegroundColor Cyan
Write-Host "  ╩╚═╩ ╩ ╩  ╩ ╚═╝╩╚═╩ ╩" -ForegroundColor Cyan
Write-Host ""

# Detect if running from remote or local
$IsRemote = $MyInvocation.MyCommand.Path -eq $null

# Determine install location
if ($User -or -not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $InstallDir = Join-Path $env:LOCALAPPDATA $AppName
    $PathScope = "User"
    Write-Host "[INFO] Installing for current user..." -ForegroundColor Blue
} else {
    $InstallDir = Join-Path $env:ProgramFiles $AppName
    $PathScope = "Machine"
    Write-Host "[INFO] Installing system-wide..." -ForegroundColor Blue
}

function Get-LatestVersion {
    try {
        $releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -ErrorAction Stop
        return $releases.tag_name -replace '^v', ''
    } catch {
        Write-Host "[WARN] Could not fetch latest version, using default: v$Version" -ForegroundColor Yellow
        return $Version
    }
}

function Get-Architecture {
    if ([Environment]::Is64BitOperatingSystem) {
        return "x86_64"
    } else {
        throw "32-bit Windows is not supported"
    }
}

function Add-ToPath {
    param([string]$Directory, [string]$Scope)

    $currentPath = [Environment]::GetEnvironmentVariable("Path", $Scope)
    if ($currentPath -notlike "*$Directory*") {
        $newPath = "$currentPath;$Directory"
        [Environment]::SetEnvironmentVariable("Path", $newPath, $Scope)
        $env:Path = "$env:Path;$Directory"
        Write-Host "[SUCCESS] Added $Directory to $Scope PATH" -ForegroundColor Green
    } else {
        Write-Host "[INFO] $Directory is already in PATH" -ForegroundColor Blue
    }
}

function Remove-FromPath {
    param([string]$Directory, [string]$Scope)

    $currentPath = [Environment]::GetEnvironmentVariable("Path", $Scope)
    $newPath = ($currentPath.Split(';') | Where-Object { $_ -ne $Directory -and $_ -ne "" }) -join ';'
    [Environment]::SetEnvironmentVariable("Path", $newPath, $Scope)
    Write-Host "[SUCCESS] Removed $Directory from $Scope PATH" -ForegroundColor Green
}

function Install-Ratterm {
    # Get latest version
    $script:Version = Get-LatestVersion
    Write-Host "[INFO] Installing version: v$Version" -ForegroundColor Blue

    # Determine download URL
    $Arch = Get-Architecture
    $AssetName = "$BinaryName-windows-$Arch.exe"
    $DownloadUrl = "https://github.com/$Repo/releases/download/v$Version/$AssetName"

    # Create install directory
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        Write-Host "[SUCCESS] Created $InstallDir" -ForegroundColor Green
    }

    # Download binary
    $DestPath = Join-Path $InstallDir "$BinaryName.exe"
    Write-Host "[INFO] Downloading from $DownloadUrl..." -ForegroundColor Blue

    try {
        # Try local file first (for local installs)
        if (-not $IsRemote) {
            $ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
            $LocalExe = Join-Path $ScriptDir "target\release\$BinaryName.exe"
            if (Test-Path $LocalExe) {
                Copy-Item -Path $LocalExe -Destination $DestPath -Force
                Write-Host "[SUCCESS] Installed from local build" -ForegroundColor Green
                return
            }
        }

        # Download from GitHub
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $DestPath -UseBasicParsing
        Write-Host "[SUCCESS] Downloaded $BinaryName.exe" -ForegroundColor Green
    } catch {
        throw "Failed to download: $_"
    }

    # Add to PATH
    Add-ToPath -Directory $InstallDir -Scope $PathScope

    Write-Host ""
    Write-Host "[SUCCESS] Installation complete!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Run 'rat' to start ratterm." -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Please restart your terminal for PATH changes to take effect." -ForegroundColor Yellow
}

function Uninstall-Ratterm {
    Write-Host "[INFO] Uninstalling $AppName..." -ForegroundColor Blue

    # Remove from PATH
    Remove-FromPath -Directory $InstallDir -Scope $PathScope

    # Remove install directory
    if (Test-Path $InstallDir) {
        Remove-Item -Path $InstallDir -Recurse -Force
        Write-Host "[SUCCESS] Removed $InstallDir" -ForegroundColor Green
    }

    Write-Host ""
    Write-Host "[SUCCESS] $AppName has been uninstalled." -ForegroundColor Green
    Write-Host "Please restart your terminal for PATH changes to take effect." -ForegroundColor Yellow
}

# Main
if ($Uninstall) {
    Uninstall-Ratterm
} else {
    Install-Ratterm
}
