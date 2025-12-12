# Test script for install.ps1
# Usage:
#   .\scripts\test-install.ps1 syntax    # Syntax check only
#   .\scripts\test-install.ps1 dry-run   # Dry run (parse and validate)
#   .\scripts\test-install.ps1 full      # Full install/uninstall test
#   .\scripts\test-install.ps1 help      # Show usage

param(
    [Parameter(Position=0)]
    [ValidateSet("", "syntax", "dry-run", "full", "help")]
    [string]$Command = ""
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir
$InstallScript = Join-Path $ProjectDir "install.ps1"

function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message" -ForegroundColor Green
}

function Write-Err {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

function Test-Syntax {
    Write-Info "Testing PowerShell syntax..."

    if (-not (Test-Path $InstallScript)) {
        Write-Err "Install script not found: $InstallScript"
        exit 1
    }

    try {
        $errors = $null
        $tokens = $null
        $null = [System.Management.Automation.Language.Parser]::ParseFile(
            $InstallScript,
            [ref]$tokens,
            [ref]$errors
        )

        if ($errors.Count -gt 0) {
            Write-Err "Syntax errors found:"
            foreach ($err in $errors) {
                Write-Host "  Line $($err.Extent.StartLineNumber): $($err.Message)"
            }
            exit 1
        }

        Write-Success "Syntax check passed"
    } catch {
        Write-Err "Failed to parse script: $_"
        exit 1
    }
}

function Test-DryRun {
    Write-Info "Testing install script (dry run)..."

    # Test that script can be dot-sourced without running
    try {
        $scriptContent = Get-Content $InstallScript -Raw

        # Check for required functions
        $requiredFunctions = @(
            "Get-LatestVersion",
            "Get-Architecture",
            "Install-Ratterm",
            "Uninstall-Ratterm",
            "Add-ToPath",
            "Remove-FromPath"
        )

        $missingFunctions = @()
        foreach ($func in $requiredFunctions) {
            if ($scriptContent -notmatch "function\s+$func") {
                $missingFunctions += $func
            }
        }

        if ($missingFunctions.Count -gt 0) {
            Write-Err "Missing required functions: $($missingFunctions -join ', ')"
            exit 1
        }

        Write-Success "All required functions found"

        # Check for required parameters
        $requiredParams = @("Uninstall", "User", "VerboseOutput")
        $missingParams = @()
        foreach ($param in $requiredParams) {
            if ($scriptContent -notmatch "\[switch\]\s*\`$$param") {
                $missingParams += $param
            }
        }

        if ($missingParams.Count -gt 0) {
            Write-Err "Missing required parameters: $($missingParams -join ', ')"
            exit 1
        }

        Write-Success "All required parameters found"

        # Check error handling
        if ($scriptContent -notmatch '\$ErrorActionPreference\s*=\s*"Stop"') {
            Write-Err "Script should set ErrorActionPreference to Stop"
            exit 1
        }

        Write-Success "Error handling configured correctly"

    } catch {
        Write-Err "Dry run failed: $_"
        exit 1
    }
}

function Test-IrmPipeExecution {
    Write-Info "Testing irm pipe execution (simulated)..."

    # Test that the install script can be executed via piped input
    # This simulates: irm <url> | iex
    # For local testing, we read the file and invoke it via ScriptBlock

    try {
        # Read the install script content
        $scriptContent = Get-Content $InstallScript -Raw

        # Test that the script can be parsed as a ScriptBlock
        $scriptBlock = [ScriptBlock]::Create($scriptContent)
        if ($null -eq $scriptBlock) {
            Write-Err "Failed to create ScriptBlock from install script"
            exit 1
        }

        Write-Success "Script can be parsed as ScriptBlock for irm | iex execution"

        # Test piping cat to pwsh -Command (alternative validation) - optional
        if (Get-Command pwsh -ErrorAction SilentlyContinue) {
            Write-Info "Testing cat | pwsh -Command execution pattern..."
            $testResult = Get-Content $InstallScript -Raw | pwsh -NoProfile -Command {
                param([Parameter(ValueFromPipeline)]$content)
                # Just validate it parses without errors
                try {
                    $null = [ScriptBlock]::Create($content)
                    Write-Output "OK"
                } catch {
                    Write-Output "FAIL: $_"
                }
            }

            if ($testResult -eq "OK") {
                Write-Success "Pipe execution pattern validated"
            } else {
                Write-Err "Pipe execution pattern failed: $testResult"
                exit 1
            }
        } else {
            Write-Info "pwsh (PowerShell 7) not available, skipping pipe execution test"
        }

    } catch {
        Write-Err "irm pipe execution test failed: $_"
        exit 1
    }
}

function Test-FullInstall {
    Write-Info "Testing full install/uninstall cycle..."

    # Create temp directory for isolated install
    $TempInstallDir = Join-Path $env:TEMP "ratterm-test-$(Get-Random)"

    Write-Info "Using temp install directory: $TempInstallDir"

    try {
        # Build the project first if not already built
        $ExePath = Join-Path $ProjectDir "target\release\rat.exe"
        if (-not (Test-Path $ExePath)) {
            Write-Info "Building release binary..."
            Push-Location $ProjectDir
            try {
                cargo build --release
                if ($LASTEXITCODE -ne 0) {
                    Write-Err "Failed to build release binary"
                    exit 1
                }
            } finally {
                Pop-Location
            }
        }

        Write-Success "Release binary available"

        # Test that we can get architecture
        Write-Info "Testing architecture detection..."
        if ([Environment]::Is64BitOperatingSystem) {
            Write-Success "Detected 64-bit OS"
        } else {
            Write-Err "32-bit OS not supported"
            exit 1
        }

        # Simulate install (copy to temp dir)
        Write-Info "Testing install to temp directory..."
        New-Item -ItemType Directory -Path $TempInstallDir -Force | Out-Null
        Copy-Item -Path $ExePath -Destination (Join-Path $TempInstallDir "rat.exe")

        if (Test-Path (Join-Path $TempInstallDir "rat.exe")) {
            Write-Success "Binary copied successfully"
        } else {
            Write-Err "Failed to copy binary"
            exit 1
        }

        # Test execution
        Write-Info "Testing binary execution..."
        $versionOutput = & (Join-Path $TempInstallDir "rat.exe") --version 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Success "Binary executes correctly: $versionOutput"
        } else {
            Write-Err "Binary execution failed"
            exit 1
        }

        # Test uninstall (cleanup)
        Write-Info "Testing uninstall (cleanup)..."
        Remove-Item -Path $TempInstallDir -Recurse -Force

        if (-not (Test-Path $TempInstallDir)) {
            Write-Success "Cleanup successful"
        } else {
            Write-Err "Cleanup failed"
            exit 1
        }

        Write-Host ""
        Write-Success "Full install/uninstall test passed!"

    } catch {
        Write-Err "Full install test failed: $_"

        # Cleanup on failure
        if (Test-Path $TempInstallDir) {
            Remove-Item -Path $TempInstallDir -Recurse -Force -ErrorAction SilentlyContinue
        }

        exit 1
    }
}

function Show-Usage {
    Write-Host "Install Script Test Tool"
    Write-Host ""
    Write-Host "Usage: .\scripts\test-install.ps1 [COMMAND]"
    Write-Host ""
    Write-Host "Commands:"
    Write-Host "  syntax    Test PowerShell syntax"
    Write-Host "  dry-run   Validate script structure without running"
    Write-Host "  full      Full install/uninstall test cycle"
    Write-Host "  help      Show this help message"
    Write-Host ""
    Write-Host "Default: Run all tests (syntax + dry-run)"
}

# Main
switch ($Command) {
    "" {
        Test-Syntax
        Test-DryRun
        Test-IrmPipeExecution
        Write-Host ""
        Write-Success "All tests passed!"
    }
    "syntax" {
        Test-Syntax
    }
    "dry-run" {
        Test-DryRun
        Test-IrmPipeExecution
    }
    "full" {
        Test-Syntax
        Test-DryRun
        Test-IrmPipeExecution
        Test-FullInstall
    }
    "help" {
        Show-Usage
    }
}
