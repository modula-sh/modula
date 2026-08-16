# Modula — full local release build (Windows / PowerShell equivalent of scripts/build.sh).
#
# Builds the engine, stages it as the desktop sidecar for the host target, then
# bundles the desktop app with the engine embedded. On Windows this produces an
# NSIS installer (target/release/bundle/nsis/*-setup.exe) that registers Modula
# in Start Menu + Add/Remove Programs (per-user). The installed app self-launches
# the bundled engine on open and places the `modula` CLI on PATH itself — there
# is no separate install step.
#
#   pwsh scripts/build.ps1            engine sidecar + desktop bundle
#   pwsh scripts/build.ps1 -Open      also reveal the bundle dir when done

[CmdletBinding()]
param(
    [switch]$Open
)

$ErrorActionPreference = 'Stop'

$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Desktop = Join-Path $Root 'apps/desktop'
$SidecarDir = Join-Path $Desktop 'src-tauri/binaries'

# tauri.conf.json's beforeBuildCommand is hardcoded to `pnpm build`, so pnpm is
# required for the bundle regardless of what else is installed.
if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
    Write-Error 'pnpm is required for the desktop bundle (tauri beforeBuildCommand uses it). Install it: https://pnpm.io/installation'
}

$Triple = (& rustc --print host-tuple).Trim()
$Ext = if ($Triple -like '*windows*') { '.exe' } else { '' }

Write-Host '[engine  ] building release binary'
& cargo build --release -p modula-engine
if ($LASTEXITCODE -ne 0) { Write-Error 'engine build failed' }

Write-Host "[sidecar ] staging engine -> binaries/modula-$Triple$Ext"
New-Item -ItemType Directory -Force -Path $SidecarDir | Out-Null
Copy-Item (Join-Path $Root "target/release/modula$Ext") (Join-Path $SidecarDir "modula-$Triple$Ext") -Force

if (-not (Test-Path (Join-Path $Desktop 'node_modules'))) {
    Write-Host '[frontend] installing deps with pnpm'
    Push-Location $Desktop
    try { & pnpm install } finally { Pop-Location }
}

Write-Host '[bundle  ] tauri build (engine bundled as sidecar)'
Push-Location $Desktop
try {
    & pnpm tauri build --config src-tauri/tauri.bundle.conf.json
    if ($LASTEXITCODE -ne 0) { Write-Error 'tauri build failed' }
} finally { Pop-Location }

$BundleDir = Join-Path $Root 'target/release/bundle'
Write-Host "[bundle  ] artifacts under: $BundleDir"
if (Test-Path $BundleDir) {
    Get-ChildItem -Path $BundleDir -Recurse -Depth 2 -Include '*.exe' -ErrorAction SilentlyContinue |
        ForEach-Object { Write-Host $_.FullName }
    if ($Open) { Invoke-Item $BundleDir }
}

Write-Host '[done    ] install the app from the bundle dir; it launches the engine on open.'
