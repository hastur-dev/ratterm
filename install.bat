@echo off
REM Ratterm Installer - Windows Batch Wrapper
REM This script runs the PowerShell installer

echo.
echo ========================================
echo        Ratterm Installer
echo ========================================
echo.

REM Check if running as admin
net session >nul 2>&1
if %errorLevel% == 0 (
    echo Running as Administrator - will install system-wide
    powershell -ExecutionPolicy Bypass -File "%~dp0install.ps1"
) else (
    echo Running as User - will install for current user only
    echo (Run as Administrator for system-wide install)
    echo.
    powershell -ExecutionPolicy Bypass -File "%~dp0install.ps1" -User
)

echo.
pause
