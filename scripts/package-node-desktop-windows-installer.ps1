param(
    [string]$Version = $env:UHORSE_VERSION,
    [string]$Target = $env:TARGET
)

$ErrorActionPreference = 'Stop'

$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

if (-not $Version) {
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    $Version = ($metadata.packages | Where-Object { $_.name -eq 'uhorse-node-desktop' } | Select-Object -First 1).version
}

if (-not $Target) {
    $Target = ((rustc -vV | Select-String '^host: ').ToString() -replace '^host:\s*', '').Trim()
}

if ($Target -notlike '*windows*') {
    throw "unsupported target for Windows installer: $Target"
}

$packageRoot = Join-Path $ProjectRoot 'target/node-desktop-package'
$payloadDir = Join-Path $packageRoot "uhorse-node-desktop-$Version-$Target"
$stagingDir = Join-Path $packageRoot "uhorse-node-desktop-$Version-$Target-windows-installer"
$installerPath = Join-Path $packageRoot "uhorse-node-desktop-$Version-$Target-installer.exe"
$launcherPath = Join-Path $stagingDir 'start-node-desktop.cmd'
$nsisScript = Join-Path $ProjectRoot 'packaging/windows/uhorse-node-desktop.nsi'

if (-not (Test-Path $payloadDir)) {
    throw "missing payload directory: $payloadDir`nrun ./scripts/package-node-desktop.sh first"
}

if (-not (Test-Path $nsisScript)) {
    throw "missing NSIS script: $nsisScript"
}

$makensis = Get-Command makensis.exe -ErrorAction SilentlyContinue
if (-not $makensis) {
    throw 'makensis.exe is required to create the Windows installer'
}

Remove-Item $stagingDir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $stagingDir | Out-Null
Copy-Item (Join-Path $payloadDir '*') $stagingDir -Recurse -Force

$launcher = @'
@echo off
setlocal
set "APP_ROOT=%~dp0"
set "BASE_URL=http://127.0.0.1:8757"
set "CONFIG_DIR=%LOCALAPPDATA%\uHorse Node Desktop\Config"
set "LOG_DIR=%LOCALAPPDATA%\uHorse Node Desktop\Logs"
set "CONFIG_PATH=%CONFIG_DIR%\node-desktop.toml"
set "LOG_PATH=%LOG_DIR%\host.log"

if not exist "%CONFIG_DIR%" mkdir "%CONFIG_DIR%" >nul 2>nul
if not exist "%LOG_DIR%" mkdir "%LOG_DIR%" >nul 2>nul

powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$uri = '%BASE_URL%/api/settings/defaults'; try { Invoke-WebRequest -UseBasicParsing -Uri $uri -TimeoutSec 2 | Out-Null; exit 0 } catch { exit 1 }"
if errorlevel 1 (
  powershell -NoProfile -ExecutionPolicy Bypass -Command ^
    "Start-Process -WindowStyle Hidden -WorkingDirectory '%APP_ROOT%' -FilePath '%APP_ROOT%bin\uhorse-node-desktop.exe' -ArgumentList '--config', '%CONFIG_PATH%', 'serve', '--listen', '127.0.0.1:8757' -RedirectStandardOutput '%LOG_PATH%' -RedirectStandardError '%LOG_PATH%'"
  powershell -NoProfile -ExecutionPolicy Bypass -Command ^
    "$uri = '%BASE_URL%/api/settings/defaults'; for ($i = 0; $i -lt 30; $i++) { try { Invoke-WebRequest -UseBasicParsing -Uri $uri -TimeoutSec 2 | Out-Null; exit 0 } catch { Start-Sleep -Seconds 1 } }; exit 1"
)

start "" "%BASE_URL%/dashboard"
exit /b 0
'@
Set-Content -Path $launcherPath -Value $launcher -Encoding ASCII

Remove-Item $installerPath -Force -ErrorAction SilentlyContinue
& $makensis.Source "/DPAYLOAD_DIR=$stagingDir" "/DOUTPUT_FILE=$installerPath" "/DVERSION=$Version" $nsisScript | Out-Host

Write-Output "payload_dir=$payloadDir"
Write-Output "installer=$installerPath"
