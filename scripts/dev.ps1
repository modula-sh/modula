# Modula — dev entrypoint (Windows / PowerShell equivalent of scripts/dev.sh).
#
# Builds + runs the engine (gRPC over the local IPC pipe), then `tauri dev`
# via the repo-pinned v2 CLI (which starts Vite on 9100 itself and opens the
# native window).

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Desktop = Join-Path $Root 'apps/desktop'

$FrontendPort = if ($env:MODULA_FRONTEND_PORT) { $env:MODULA_FRONTEND_PORT } else { '9100' }
$env:MODULA_FRONTEND_PORT = $FrontendPort

if (Get-Command pnpm -ErrorAction SilentlyContinue) {
    $Js = 'pnpm'
} elseif (Get-Command npm -ErrorAction SilentlyContinue) {
    $Js = 'npm'
} else {
    Write-Error 'Need pnpm or npm in PATH'
}

if (-not (Test-Path (Join-Path $Desktop 'node_modules'))) {
    Write-Host "[setup   ] frontend: installing deps with $Js"
    Push-Location $Desktop
    try { & $Js install } finally { Pop-Location }
}

$Jobs = @()
function Stop-Jobs {
    foreach ($j in $script:Jobs) {
        if ($j -and -not $j.HasExited) {
            # /T tears down the child tree (cmd → tauri → vite) so port 9100 frees up.
            try { & taskkill /PID $j.Id /T /F 2>&1 | Out-Null } catch { }
        }
    }
}

try {
    Write-Host '[engine  ] building'
    & cargo build --release -p modula-engine
    if ($LASTEXITCODE -ne 0) { Write-Error 'engine build failed' }

    # Put the freshly built CLI on PATH every dev launch (production links itself
    # only on update). Best-effort: a link failure must not block the engine.
    Write-Host '[cli     ] linking modula onto PATH'
    & (Join-Path $Root 'target/release/modula.exe') link-cli
    if ($LASTEXITCODE -ne 0) { Write-Warning 'could not link modula onto PATH' }

    $Pipe = if ($env:MODULA_ENGINE_SOCKET) { $env:MODULA_ENGINE_SOCKET } else { '\\.\pipe\modula-engine-<user-hash>' }
    Write-Host "[engine  ] starting on $Pipe"
    $engineExe = Join-Path $Root 'target/release/modula.exe'
    $Jobs += Start-Process -FilePath $engineExe -ArgumentList @('engine') -NoNewWindow -PassThru

    Write-Host "[tauri   ] starting native shell (Vite on $FrontendPort via beforeDevCommand)"
    $env:MODULA_DEV = '1'
    # Launch via cmd.exe: Start-Process -NoNewWindow uses CreateProcess, which
    # can't run the `pnpm`/`npm` shim (a .cmd, not a Win32 .exe) directly.
    $Jobs += Start-Process -FilePath $env:ComSpec -ArgumentList @('/c', $Js, 'exec', 'tauri', 'dev') -WorkingDirectory $Desktop -NoNewWindow -PassThru

    # Wait until any child exits, then tear the rest down.
    while ($true) {
        if ($Jobs | Where-Object { $_.HasExited }) { break }
        Start-Sleep -Seconds 1
    }
} finally {
    Stop-Jobs
}
