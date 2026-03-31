$ErrorActionPreference = 'Stop'

param(
    [string]$InstallRoot = $env:DESKTOP_INSTALL_ROOT,
    [string]$Listen = $env:DESKTOP_SMOKE_LISTEN
)

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
        Fail '用法：./scripts/desktop-installer-smoke.ps1 -InstallRoot <install-root>'
    }

    New-Item -ItemType Directory -Path $RuntimeDir | Out-Null

    $BinPath = Join-Path $InstallRoot 'bin/uhorse-node-desktop.exe'
    $IndexPath = Join-Path $InstallRoot 'web/index.html'
    $AssetsPath = Join-Path $InstallRoot 'web/assets'

    if (-not (Test-Path $BinPath -PathType Leaf)) {
        Fail "未找到安装后的宿主二进制：$BinPath"
    }
    if (-not (Test-Path $IndexPath -PathType Leaf)) {
        Fail "未找到安装后的 web/index.html：$IndexPath"
    }
    if (-not (Test-Path $AssetsPath -PathType Container)) {
        Fail "未找到安装后的 web/assets：$AssetsPath"
    }

    $ProjectRootToml = $ProjectRoot.Replace('\', '\\')

    @"
name = "Desktop Installer Smoke"
workspace_path = "$ProjectRootToml"
require_git_repo = false

[connection]
hub_url = "ws://localhost:8765/ws"
"@ | Set-Content -Path $ConfigPath -Encoding Ascii

    Info '启动安装后的 Node Desktop 宿主...'
    $process = Start-Process -FilePath $BinPath -WorkingDirectory $InstallRoot -ArgumentList @('--config', $ConfigPath, 'serve', '--listen', $Listen) -RedirectStandardOutput $StdoutLogPath -RedirectStandardError $StderrLogPath -PassThru -WindowStyle Hidden

    $started = $false
    for ($i = 0; $i -lt 30; $i++) {
        try {
            $null = Invoke-Json "$BaseUrl/api/settings/defaults"
            Pass '安装后的宿主 API 已启动'
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
        Fail '安装后的宿主 API 启动失败'
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
            Fail "API 返回 success != true：$path"
        }
    }
    Pass '安装后的关键 API smoke 通过'

    $indexHtml = Invoke-Text "$BaseUrl/"
    if ($indexHtml -match 'id="root"') {
        Pass '安装后的静态首页可访问'
    }
    else {
        Fail '安装后的静态首页内容不符合预期'
    }

    $appHtml = Invoke-Text "$BaseUrl/dashboard"
    if ($appHtml -match 'id="root"') {
        Pass '安装后的前端路由回退可访问'
    }
    else {
        Fail '安装后的前端路由回退不可用'
    }

    Write-Host ''
    Write-Host "Node Desktop installer smoke 完成。日志：$StdoutLogPath , $StderrLogPath"
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
