# Rebuilds the debug app with the webview debugging port open (for tools/cdp.mjs) and starts it.
# The interface is served by `npm run dev`, which has to be running.
$ErrorActionPreference = 'Stop'
$env:PATH = "$env:USERPROFILE\.cargo\bin;" + $env:PATH
Set-Location (Join-Path $PSScriptRoot '..\src-tauri')

Get-Process quadra -ErrorAction SilentlyContinue | Where-Object { $_.Path -like '*\target\debug\*' } | Stop-Process -Force
$conf = Get-Content tauri.conf.json -Raw | ConvertFrom-Json
$window = $conf.app.windows[0]
$window | Add-Member -NotePropertyName additionalBrowserArgs -Force `
    -NotePropertyValue '--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port=9222'
# its own identifier: separate settings folder and no clash with a running release copy
$env:TAURI_CONFIG = (@{ identifier = 'dev.quadra.client.test'; app = @{ windows = @($window) } } | ConvertTo-Json -Depth 6 -Compress)
(Get-Process -Id $PID).PriorityClass = 'BelowNormal'
$ErrorActionPreference = 'Continue'
cargo build -j 6 --message-format short 2>&1 | Where-Object { $_ -notmatch '^\s*(Compiling|Downloading|Downloaded|Locking|Adding)' } | Select-Object -Last 25
$built = $LASTEXITCODE -eq 0
Remove-Item Env:TAURI_CONFIG
if ($built) {
    if ($args.Count) { Start-Process -FilePath .\target\debug\quadra.exe -WorkingDirectory . -ArgumentList $args }
    else { Start-Process -FilePath .\target\debug\quadra.exe -WorkingDirectory . }
    Start-Sleep -Seconds 6
    'started'
} else {
    'BUILD FAILED'
}
