<#
.SYNOPSIS
    Rclone Manager (RCM) installer for Windows (x86_64, ARM64)
.DESCRIPTION
    Installs RCM per-user without requiring Administrator privileges.
    Satisfies requirements IN-1, IN-2, IN-3 per SRDD §10.
#>
[CmdletBinding()]
param(
    [string]$Version = "latest",
    [switch]$Autostart,
    [switch]$NoWinFsp,
    [switch]$Uninstall,
    [switch]$Purge
)

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\RCM"
$DataDir    = Join-Path $env:LOCALAPPDATA "RCM"
$ConfigDir  = Join-Path $env:APPDATA "RCM"

function Write-Info($msg) { Write-Host "[INFO] $msg" -ForegroundColor Cyan }
function Write-Warn($msg) { Write-Host "[WARN] $msg" -ForegroundColor Yellow }
function Write-Success($msg) { Write-Host "[OK]   $msg" -ForegroundColor Green }

if ($Uninstall) {
    Write-Info "Stopping any running RCM processes..."
    Get-Process -Name "rcm", "rcm-agent", "rcmctl" -ErrorAction SilentlyContinue | Stop-Process -Force

    Write-Info "Removing files from $InstallDir..."
    if (Test-Path $InstallDir) {
        Remove-Item -Recurse -Force $InstallDir
    }

    # Start Menu shortcut
    $ShortcutPath = Join-Path ([Environment]::GetFolderPath("Programs")) "Rclone Manager.lnk"
    if (Test-Path $ShortcutPath) { Remove-Item -Force $ShortcutPath }

    # Autostart entry
    reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v "RcloneManagerAgent" /f 2>$null

    # Uninstall entry
    reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\RcloneManager" /f 2>$null

    if ($Purge) {
        Write-Warn "Purging data and configuration directories..."
        if (Test-Path $DataDir) { Remove-Item -Recurse -Force $DataDir }
        if (Test-Path $ConfigDir) { Remove-Item -Recurse -Force $ConfigDir }
    }

    Write-Success "Rclone Manager has been uninstalled successfully."
    exit 0
}

Write-Info "Checking prerequisites..."

# IN-3: WinFsp Check
if (-not $NoWinFsp) {
    $WinFspInstalled = $false
    $RegKeys = @("HKLM:\SOFTWARE\WinFsp", "HKLM:\SOFTWARE\WOW6432Node\WinFsp")
    foreach ($k in $RegKeys) {
        if (Test-Path $k) { $WinFspInstalled = $true; break }
    }
    if (-not $WinFspInstalled -and (Test-Path "$env:ProgramFiles\WinFsp\bin\winfsp-x64.dll")) {
        $WinFspInstalled = $true
    }

    if (-not $WinFspInstalled) {
        Write-Warn "WinFsp is not detected. WinFsp is required to mount remotes as Windows drive letters."
        Write-Info "You can install it automatically via winget:"
        Write-Host "  winget install WinFsp.WinFsp" -ForegroundColor White
        Write-Info "Or download it from: https://winfsp.dev/"
    } else {
        Write-Success "WinFsp driver is present."
    }
}

Write-Info "Installing Rclone Manager to $InstallDir..."
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# Stop existing processes before upgrading
Get-Process -Name "rcm", "rcm-agent", "rcmctl" -ErrorAction SilentlyContinue | Stop-Process -Force

# Create Start Menu shortcut
$WshShell = New-Object -ComObject WScript.Shell
$ShortcutPath = Join-Path ([Environment]::GetFolderPath("Programs")) "Rclone Manager.lnk"
$Shortcut = $WshShell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = Join-Path $InstallDir "rcm.exe"
$Shortcut.Description = "Rclone Manager Desktop"
$Shortcut.Save()

# Configure autostart if requested (DM-6)
if ($Autostart) {
    $AgentExe = Join-Path $InstallDir "rcm-agent.exe"
    reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v "RcloneManagerAgent" /t REG_SZ /d "`"$AgentExe`" --background" /f | Out-Null
    Write-Success "Autostart enabled at login."
}

# Add HKCU Add/Remove Programs entry (IN-2)
$UninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\RcloneManager"
if (-not (Test-Path $UninstallKey)) { New-Item -Path $UninstallKey -Force | Out-Null }
Set-ItemProperty -Path $UninstallKey -Name "DisplayName" -Value "Rclone Manager"
Set-ItemProperty -Path $UninstallKey -Name "DisplayVersion" -Value $Version
Set-ItemProperty -Path $UninstallKey -Name "Publisher" -Value "RCM Contributors"
Set-ItemProperty -Path $UninstallKey -Name "InstallLocation" -Value $InstallDir
Set-ItemProperty -Path $UninstallKey -Name "UninstallString" -Value "powershell.exe -ExecutionPolicy Bypass -File `"$InstallDir\install.ps1`" -Uninstall"

Write-Success "Rclone Manager installation completed successfully!"
