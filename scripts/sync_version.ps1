# sync_version.ps1 - Syncs local version to be ahead of GitHub latest release
# Usage: .\scripts\sync_version.ps1

param(
    [switch]$DryRun = $false
)

$ErrorActionPreference = "Stop"

# GitHub repo info
$Owner = "Hastur-Dev"
$Repo = "ratterm"

Write-Host "[sync_version] Checking GitHub for latest release..." -ForegroundColor Cyan

try {
    # Try to get latest release tag from GitHub API
    $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$Owner/$Repo/releases/latest" -Headers @{
        "Accept" = "application/vnd.github.v3+json"
        "User-Agent" = "ratterm-version-sync"
    } -TimeoutSec 10

    $latestTag = $response.tag_name -replace '^v', ''
    Write-Host "[sync_version] Latest GitHub release: v$latestTag" -ForegroundColor Green
}
catch {
    # If no releases exist or API fails, try tags
    try {
        $tagsResponse = Invoke-RestMethod -Uri "https://api.github.com/repos/$Owner/$Repo/tags" -Headers @{
            "Accept" = "application/vnd.github.v3+json"
            "User-Agent" = "ratterm-version-sync"
        } -TimeoutSec 10

        if ($tagsResponse.Count -gt 0) {
            $latestTag = $tagsResponse[0].name -replace '^v', ''
            Write-Host "[sync_version] Latest GitHub tag: v$latestTag" -ForegroundColor Green
        } else {
            $latestTag = "0.0.0"
            Write-Host "[sync_version] No GitHub releases/tags found, starting from 0.0.0" -ForegroundColor Yellow
        }
    }
    catch {
        Write-Host "[sync_version] Could not reach GitHub API, skipping version sync" -ForegroundColor Yellow
        exit 0
    }
}

# Parse version components
$versionParts = $latestTag -split '\.'
$major = [int]$versionParts[0]
$minor = [int]$versionParts[1]
$patch = [int]$versionParts[2]

# Increment patch version
$newPatch = $patch + 1
$newVersion = "$major.$minor.$newPatch"

Write-Host "[sync_version] New version will be: v$newVersion" -ForegroundColor Cyan

# Read current Cargo.toml
$cargoPath = Join-Path $PSScriptRoot "..\Cargo.toml"
$cargoLines = Get-Content $cargoPath

# Find the package version line (should be in [package] section, early in file)
$inPackageSection = $false
$versionLineIndex = -1
$currentVersion = $null

for ($i = 0; $i -lt $cargoLines.Count; $i++) {
    $line = $cargoLines[$i]

    if ($line -match '^\[package\]') {
        $inPackageSection = $true
        continue
    }

    if ($line -match '^\[' -and $line -notmatch '^\[package\]') {
        $inPackageSection = $false
    }

    if ($inPackageSection -and $line -match '^version\s*=\s*"([^"]+)"') {
        $currentVersion = $matches[1]
        $versionLineIndex = $i
        break
    }
}

if ($versionLineIndex -eq -1) {
    Write-Host "[sync_version] ERROR: Could not find version in [package] section" -ForegroundColor Red
    exit 1
}

Write-Host "[sync_version] Current Cargo.toml version: v$currentVersion" -ForegroundColor Gray

# Compare versions
$currentParts = $currentVersion -split '\.'
$currentMajor = [int]$currentParts[0]
$currentMinor = [int]$currentParts[1]
$currentPatch = [int]$currentParts[2]

# Only update if GitHub version is >= current version
$githubNewer = ($major -gt $currentMajor) -or
               (($major -eq $currentMajor) -and ($minor -gt $currentMinor)) -or
               (($major -eq $currentMajor) -and ($minor -eq $currentMinor) -and ($patch -ge $currentPatch))

if (-not $githubNewer) {
    Write-Host "[sync_version] Local version ($currentVersion) is already ahead of GitHub ($latestTag)" -ForegroundColor Green
    exit 0
}

if ($DryRun) {
    Write-Host "[sync_version] DRY RUN - Would update to v$newVersion" -ForegroundColor Yellow
    exit 0
}

# Update only the package version line
$cargoLines[$versionLineIndex] = "version = `"$newVersion`""
$cargoLines | Set-Content $cargoPath

Write-Host "[sync_version] Updated Cargo.toml to v$newVersion" -ForegroundColor Green
Write-Host "[sync_version] Version sync complete!" -ForegroundColor Green
