# Local CI testing script using Docker
# Mirrors the GitHub Actions CI environment
#
# Usage:
#   .\scripts\test-local.ps1          # Run all CI checks
#   .\scripts\test-local.ps1 fmt      # Format check only
#   .\scripts\test-local.ps1 clippy   # Clippy lints only
#   .\scripts\test-local.ps1 test     # Tests only
#   .\scripts\test-local.ps1 docs     # Documentation only
#   .\scripts\test-local.ps1 audit    # Security audit only
#   .\scripts\test-local.ps1 msrv     # MSRV check only
#   .\scripts\test-local.ps1 clean    # Clean up Docker volumes

param(
    [Parameter(Position=0)]
    [ValidateSet("", "fmt", "clippy", "test", "docs", "audit", "msrv", "ci-all", "install-test", "lua-test", "lua-test-arm", "test-arm", "ci-all-arm", "clean", "help")]
    [string]$Command = ""
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir
$DockerDir = Join-Path $ProjectDir "docker"

function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message" -ForegroundColor Green
}

function Write-Warn {
    param([string]$Message)
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

function Write-Err {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

function Test-Docker {
    Write-Info "Checking Docker availability..."

    try {
        $null = Get-Command docker -ErrorAction Stop
    } catch {
        Write-Err "Docker is not installed or not in PATH"
        Write-Host "Please install Docker: https://docs.docker.com/get-docker/"
        exit 1
    }

    # Check if Docker daemon is running
    $dockerInfo = docker info 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Err "Docker daemon is not running"
        Write-Host "Please start Docker Desktop and try again"
        Write-Host "Debug: $dockerInfo"
        exit 1
    }

    Write-Info "Docker is available"
}

function Invoke-Service {
    param([string]$Service)

    Write-Info "Running: $Service"

    Push-Location $ProjectDir
    try {
        docker compose -f docker/docker-compose.yml up --build --abort-on-container-exit $Service
        $exitCode = $LASTEXITCODE

        if ($exitCode -eq 0) {
            Write-Success "$Service passed"
        } else {
            Write-Err "$Service failed with exit code $exitCode"
            exit $exitCode
        }
    } finally {
        Pop-Location
    }
}

function Invoke-AllChecks {
    Write-Info "Running all CI checks..."
    Write-Host ""

    $checks = @("fmt", "clippy", "test", "docs", "audit", "msrv")
    $failed = @()

    foreach ($check in $checks) {
        Write-Host ""
        Write-Host "=========================================="
        Write-Info "Running: $check"
        Write-Host "=========================================="

        Push-Location $ProjectDir
        try {
            docker compose -f docker/docker-compose.yml up --build --abort-on-container-exit $check
            if ($LASTEXITCODE -eq 0) {
                Write-Success "$check passed"
            } else {
                Write-Err "$check failed"
                $failed += $check
            }
        } finally {
            Pop-Location
        }
    }

    Write-Host ""
    Write-Host "=========================================="
    Write-Host "                SUMMARY                   "
    Write-Host "=========================================="

    if ($failed.Count -eq 0) {
        Write-Success "All checks passed!"
    } else {
        Write-Err "Failed checks: $($failed -join ', ')"
        exit 1
    }
}

function Invoke-Clean {
    Write-Info "Cleaning up Docker resources..."

    Push-Location $ProjectDir
    try {
        docker compose -f docker/docker-compose.yml down -v --rmi local
        Write-Success "Cleanup complete"
    } finally {
        Pop-Location
    }
}

function Show-Usage {
    Write-Host "Usage: .\scripts\test-local.ps1 [COMMAND]"
    Write-Host ""
    Write-Host "Commands:"
    Write-Host "  (none)       Run all CI checks"
    Write-Host "  fmt          Format check only"
    Write-Host "  clippy       Clippy lints only"
    Write-Host "  test         Tests only"
    Write-Host "  docs         Documentation only"
    Write-Host "  audit        Security audit only"
    Write-Host "  msrv         MSRV check only"
    Write-Host "  ci-all       Run all checks in one container"
    Write-Host "  install-test Test install script in Docker"
    Write-Host "  lua-test     Run Lua extension tests"
    Write-Host "  lua-test-arm Run Lua extension tests on ARM64 (QEMU)"
    Write-Host "  test-arm     Run tests on ARM64 Linux (QEMU)"
    Write-Host "  ci-all-arm   Run all checks on ARM64 Linux (QEMU)"
    Write-Host "  clean        Clean up Docker volumes and images"
    Write-Host "  help         Show this help message"
}

# Main
Test-Docker

switch ($Command) {
    "" {
        Invoke-AllChecks
    }
    "fmt" { Invoke-Service "fmt" }
    "clippy" { Invoke-Service "clippy" }
    "test" { Invoke-Service "test" }
    "docs" { Invoke-Service "docs" }
    "audit" { Invoke-Service "audit" }
    "msrv" { Invoke-Service "msrv" }
    "ci-all" { Invoke-Service "ci-all" }
    "install-test" { Invoke-Service "install-test" }
    "lua-test" { Invoke-Service "lua-test" }
    "lua-test-arm" { Invoke-Service "lua-test-arm" }
    "test-arm" { Invoke-Service "test-arm" }
    "ci-all-arm" { Invoke-Service "ci-all-arm" }
    "clean" { Invoke-Clean }
    "help" { Show-Usage }
    default {
        Write-Err "Unknown command: $Command"
        Show-Usage
        exit 1
    }
}
