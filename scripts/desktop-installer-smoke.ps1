param(
    [string]$InstallRoot = $env:DESKTOP_INSTALL_ROOT,
    [string]$Listen = $env:DESKTOP_SMOKE_LISTEN
)

$ErrorActionPreference = 'Stop'

if (-not $Listen) {
    $Listen = '127.0.0.1:8757'
}

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$BaseUrl = "http://$Listen"
$RuntimeDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
$ConfigPath = Join-Path $RuntimeDir 'node-desktop.toml'
$StdoutLogPath = Join-Path $RuntimeDir 'node-desktop-installer.stdout.log'
$StderrLogPath = Join-Path $RuntimeDir 'node-desktop-installer.stderr.log'
$process = $null

function Pass([string]$Message) {
    Write-Host "[ok] $Message"
}

function Info([string]$Message) {
    Write-Host "[info] $Message"
}

function Fail([string]$Message) {
    throw $Message
}

function Invoke-Json([string]$Uri) {
    Invoke-RestMethod -Uri $Uri -TimeoutSec 2
}

function Invoke-Text([string]$Uri) {
    (Invoke-WebRequest -Uri $Uri -UseBasicParsing -TimeoutSec 2).Content
}

try {
    if (-not $InstallRoot) {
        Fail 'usage: ./scripts/desktop-installer-smoke.ps1 -InstallRoot <install-root>'
    }

    New-Item -ItemType Directory -Path $RuntimeDir | Out-Null

    $BinPath = Join-Path $InstallRoot 'bin/uhorse-node-desktop.exe'
    $IndexPath = Join-Path $InstallRoot 'web/index.html'
    $AssetsPath = Join-Path $InstallRoot 'web/assets'

    if (-not (Test-Path $BinPath -PathType Leaf)) {
        Fail "missing installed host binary: $BinPath"
    }
    if (-not (Test-Path $IndexPath -PathType Leaf)) {
        Fail "missing installed web/index.html: $IndexPath"
    }
    if (-not (Test-Path $AssetsPath -PathType Container)) {
        Fail "missing installed web/assets: $AssetsPath"
    }

    $ProjectRootToml = $ProjectRoot.Replace('\', '\\')

    @"
name = "Desktop Installer Smoke"
workspace_path = "$ProjectRootToml"
require_git_repo = false

[connection]
hub_url = "ws://localhost:8765/ws"
"@ | Set-Content -Path $ConfigPath -Encoding Ascii

    Info 'Starting installed Node Desktop host...'
    $process = Start-Process -FilePath $BinPath -WorkingDirectory $InstallRoot -ArgumentList @('--config', $ConfigPath, 'serve', '--listen', $Listen) -RedirectStandardOutput $StdoutLogPath -RedirectStandardError $StderrLogPath -PassThru -WindowStyle Hidden

    $started = $false
    for ($i = 0; $i -lt 30; $i++) {
        try {
            $null = Invoke-Json "$BaseUrl/api/settings/defaults"
            Pass 'Installed host API started'
            $started = $true
            break
        }
        catch {
            Start-Sleep -Seconds 1
        }
    }

    if (-not $started) {
        if (Test-Path $StdoutLogPath) {
            Get-Content $StdoutLogPath | Write-Host
        }
        if (Test-Path $StderrLogPath) {
            Get-Content $StderrLogPath | Write-Host
        }
        Fail 'Installed host API failed to start'
    }

    foreach ($path in @(
        '/api/settings/defaults',
        '/api/settings/capabilities',
        '/api/workspace/status',
        '/api/runtime/status',
        '/api/versioning/summary'
    )) {
        $payload = Invoke-Json ($BaseUrl + $path)
        if ($payload.success -ne $true) {
            Fail "API returned success != true: $path"
        }
    }
    Pass 'Installed key API smoke passed'

    $indexHtml = Invoke-Text "$BaseUrl/"
    if ($indexHtml -match 'id="root"') {
        Pass 'Installed static homepage reachable'
    }
    else {
        Fail 'Installed static homepage content mismatch'
    }

    $appHtml = Invoke-Text "$BaseUrl/dashboard"
    if ($appHtml -match 'id="root"') {
        Pass 'Installed frontend route fallback reachable'
    }
    else {
        Fail 'Installed frontend route fallback unavailable'
    }

    Write-Host ''
    Write-Host "Node Desktop installer smoke completed. Logs: $StdoutLogPath , $StderrLogPath"
}
finally {
    if ($process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        $process.WaitForExit()
    }
    if (Test-Path $RuntimeDir) {
        Remove-Item $RuntimeDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
