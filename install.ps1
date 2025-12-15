# Ratterm Installer for Windows
# Usage: irm https://raw.githubusercontent.com/hastur-dev/ratterm/master/install.ps1 | iex
# Debug: $env:VERBOSE="true"; irm https://raw.githubusercontent.com/hastur-dev/ratterm/master/install.ps1 | iex
# Or: .\install.ps1 [-Uninstall] [-User] [-Verbose]

param(
    [switch]$Uninstall,
    [switch]$User,
    [Alias('v')]
    [switch]$VerboseOutput
)

$ErrorActionPreference = "Stop"
$Version = "0.1.2"
$Repo = "hastur-dev/ratterm"
$BinaryName = "rat"
$AppName = "ratterm"

# Enable verbose mode from env var or parameter
$IsVerbose = $VerboseOutput -or ($env:VERBOSE -eq "true")

# Logging functions (colors removed for compatibility)
function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message"
}

function Write-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message"
}

function Write-Warn {
    param([string]$Message)
    Write-Host "[WARN] $Message"
}

function Write-Err {
    param([string]$Message)
    Write-Host "[ERROR] $Message"
}

function Write-Debug {
    param([string]$Message)
    if ($IsVerbose) {
        Write-Host "[DEBUG] $Message"
    }
}

function Write-SystemInfo {
    Write-Debug "=== System Information ==="
    Write-Debug "Date: $(Get-Date)"
    Write-Debug "PowerShell Version: $($PSVersionTable.PSVersion)"
    Write-Debug "OS: $([System.Environment]::OSVersion.VersionString)"
    $archName = if ([System.Environment]::Is64BitOperatingSystem) { 'x64' } else { 'x86' }
    Write-Debug "Architecture: $archName"
    Write-Debug "User: $env:USERNAME"
    Write-Debug "Home: $env:USERPROFILE"
    Write-Debug "PWD: $(Get-Location)"
    Write-Debug "Install Dir: $InstallDir"
    Write-Debug "PATH: $env:PATH"
    Write-Debug "=== End System Information ==="
}

# ASCII art banner
Write-Host ""
Write-Host "  RATTERM"
Write-Host ""

# Detect if running from remote or local
$ScriptPath = $MyInvocation.MyCommand.Path
$IsRemote = $ScriptPath -eq $null

# Determine install location
if ($User -or -not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $InstallDir = Join-Path $env:LOCALAPPDATA $AppName
    $PathScope = "User"
    Write-Info "Installing for current user..."
} else {
    $InstallDir = Join-Path $env:ProgramFiles $AppName
    $PathScope = "Machine"
    Write-Info "Installing system-wide..."
}

# Log system info if verbose
if ($IsVerbose) {
    Write-Info "Verbose mode enabled"
    Write-SystemInfo
}

function Get-LatestVersion {
    Write-Debug "Fetching latest version from GitHub API..."
    $ApiUrl = "https://api.github.com/repos/$Repo/releases/latest"
    Write-Debug "API URL: $ApiUrl"

    try {
        $response = Invoke-RestMethod -Uri $ApiUrl -ErrorAction Stop
        Write-Debug "API response received"
        $latestVersion = $response.tag_name -replace '^v', ''
        Write-Debug "Parsed version: '$latestVersion'"
        Write-Info "Latest version: v$latestVersion"
        return $latestVersion
    } catch {
        Write-Debug "API request failed: $_"
        Write-Warn "Could not fetch latest version, using default: v$Version"
        return $Version
    }
}

function Get-Architecture {
    Write-Debug "Detecting architecture..."
    if ([Environment]::Is64BitOperatingSystem) {
        Write-Debug "Detected: x86_64"
        return "x86_64"
    } else {
        Write-Err "32-bit Windows is not supported"
        throw "32-bit Windows is not supported"
    }
}

function Add-ToPath {
    param([string]$Directory, [string]$Scope)

    Write-Debug "Adding to PATH: $Directory (Scope: $Scope)"
    $currentPath = [Environment]::GetEnvironmentVariable("Path", $Scope)
    if ($currentPath -notlike "*$Directory*") {
        $newPath = "$currentPath;$Directory"
        [Environment]::SetEnvironmentVariable("Path", $newPath, $Scope)
        $env:Path = "$env:Path;$Directory"
        Write-Success "Added $Directory to $Scope PATH"
    } else {
        Write-Debug "Directory already in PATH"
        Write-Info "$Directory is already in PATH"
    }
}

function Remove-FromPath {
    param([string]$Directory, [string]$Scope)

    Write-Debug "Removing from PATH: $Directory (Scope: $Scope)"
    $currentPath = [Environment]::GetEnvironmentVariable("Path", $Scope)
    $newPath = ($currentPath.Split(';') | Where-Object { $_ -ne $Directory -and $_ -ne "" }) -join ';'
    [Environment]::SetEnvironmentVariable("Path", $newPath, $Scope)
    Write-Success "Removed $Directory from $Scope PATH"
}

function Install-Ratterm {
    # Get latest version
    $script:Version = Get-LatestVersion
    Write-Info "Installing version: v$Version"

    # Determine download URL
    $Arch = Get-Architecture
    $AssetName = "$BinaryName-windows-$Arch.exe"
    $DownloadUrl = "https://github.com/$Repo/releases/download/v$Version/$AssetName"

    Write-Debug "Asset name: $AssetName"
    Write-Debug "Download URL: $DownloadUrl"

    # Create install directory
    if (-not (Test-Path $InstallDir)) {
        Write-Debug "Creating install directory: $InstallDir"
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        Write-Success "Created $InstallDir"
    } else {
        Write-Debug "Install directory already exists"
    }

    # Download binary
    $DestPath = Join-Path $InstallDir "$BinaryName.exe"
    Write-Debug "Destination path: $DestPath"
    Write-Info "Downloading from $DownloadUrl..."

    try {
        # Try local file first (for local installs)
        if (-not $IsRemote -and $ScriptPath) {
            $ScriptDir = Split-Path -Parent $ScriptPath
            $LocalExe = Join-Path $ScriptDir "target\release\$BinaryName.exe"
            Write-Debug "Checking for local build: $LocalExe"
            if (Test-Path $LocalExe) {
                Copy-Item -Path $LocalExe -Destination $DestPath -Force
                Write-Success "Installed from local build"
                Add-ToPath -Directory $InstallDir -Scope $PathScope
                Write-Host ""
                Write-Success "Installation complete!"
                Write-Host ""
                Write-Host "Run 'rat' to start ratterm."
                return
            }
        }

        # Download from GitHub
        Write-Debug "Starting download..."
        $tempFile = Join-Path $env:TEMP "ratterm-download.exe"
        Write-Debug "Temp file: $tempFile"

        Invoke-WebRequest -Uri $DownloadUrl -OutFile $tempFile -UseBasicParsing

        # Verify download
        if (-not (Test-Path $tempFile)) {
            Write-Err "Download failed: temp file does not exist"
            throw "Download failed: temp file does not exist"
        }

        $fileSize = (Get-Item $tempFile).Length
        Write-Debug "Downloaded file size: $fileSize bytes"

        if ($fileSize -lt 1000) {
            $content = Get-Content $tempFile -Raw -ErrorAction SilentlyContinue
            Write-Debug "File contents (might be error): $content"
            Remove-Item $tempFile -Force -ErrorAction SilentlyContinue
            Write-Err "Download failed: file too small, likely an error page"
            throw "Download failed: file too small"
        }

        # Move to destination
        Write-Debug "Moving $tempFile to $DestPath"
        Move-Item -Path $tempFile -Destination $DestPath -Force
        Write-Success "Downloaded $BinaryName.exe"

    } catch {
        Write-Err "Failed to download: $_"
        throw "Failed to download: $_"
    }

    # Add to PATH
    Add-ToPath -Directory $InstallDir -Scope $PathScope

    # Verify installation
    Write-Debug "Verifying installation..."
    if (Test-Path $DestPath) {
        Write-Debug "Binary exists at $DestPath"
        try {
            $versionOutput = & $DestPath --version 2>&1
            Write-Debug "Version output: $versionOutput"
        } catch {
            Write-Debug "Version check failed: $_"
        }
    } else {
        Write-Err "Installation verification failed: $DestPath not found"
        throw "Installation verification failed"
    }

    Write-Host ""
    Write-Success "Installation complete!"
    Write-Host ""
    Write-Host "Run 'rat' to start ratterm."
    Write-Host ""
    Write-Warn "Please restart your terminal for PATH changes to take effect."
}

function Uninstall-Ratterm {
    Write-Info "Uninstalling $AppName..."
    Write-Debug "Install directory: $InstallDir"
    Write-Debug "PATH scope: $PathScope"

    # Remove from PATH
    Remove-FromPath -Directory $InstallDir -Scope $PathScope

    # Remove install directory
    if (Test-Path $InstallDir) {
        Write-Debug "Removing directory: $InstallDir"
        Remove-Item -Path $InstallDir -Recurse -Force
        Write-Success "Removed $InstallDir"
    } else {
        Write-Warn "$AppName is not installed at $InstallDir"
    }

    Write-Host ""
    Write-Success "$AppName has been uninstalled."
    Write-Warn "Please restart your terminal for PATH changes to take effect."
}

# Main
if ($Uninstall) {
    Uninstall-Ratterm
} else {
    Install-Ratterm
}
