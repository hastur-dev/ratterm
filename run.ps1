# run.ps1 - Syncs version with GitHub and runs the application
# Usage: .\run.ps1 [cargo args...]
# Examples:
#   .\run.ps1                    # cargo run --release
#   .\run.ps1 --debug            # cargo run (debug mode)
#   .\run.ps1 --skip-sync        # Skip version sync

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
$cargoCmd = @("run")
if (-not $Debug) {
    $cargoCmd += "--release"
}
if ($CargoArgs) {
    $cargoCmd += $CargoArgs
}

Write-Host "[run] Executing: cargo $($cargoCmd -join ' ')" -ForegroundColor Cyan
& cargo @cargoCmd
