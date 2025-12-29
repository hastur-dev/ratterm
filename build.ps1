# build.ps1 - Syncs version with GitHub and builds the application
# Usage: .\build.ps1 [options]
# Examples:
#   .\build.ps1                  # cargo build --release
#   .\build.ps1 --debug          # cargo build (debug mode)
#   .\build.ps1 --skip-sync      # Skip version sync

param(
    [switch]$SkipSync = $false,
    [switch]$Debug = $false,
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$CargoArgs
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

# Sync version unless skipped
if (-not $SkipSync) {
    Write-Host ""
    & "$PSScriptRoot\scripts\sync_version.ps1"
    Write-Host ""
}

# Build cargo command
$cargoCmd = @("build")
if (-not $Debug) {
    $cargoCmd += "--release"
}
if ($CargoArgs) {
    $cargoCmd += $CargoArgs
}

Write-Host "[build] Executing: cargo $($cargoCmd -join ' ')" -ForegroundColor Cyan
& cargo @cargoCmd

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "[build] Build successful!" -ForegroundColor Green

    # Show binary location
    if ($Debug) {
        $binPath = "target\debug\rat.exe"
    } else {
        $binPath = "target\release\rat.exe"
    }

    if (Test-Path $binPath) {
        $size = (Get-Item $binPath).Length / 1MB
        Write-Host "[build] Binary: $binPath ($([math]::Round($size, 2)) MB)" -ForegroundColor Cyan
    }
}
