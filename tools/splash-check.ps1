# Starts the debug app and photographs the middle of the screen while the startup cube is up,
# to check that its window really is transparent. Needs tools/dev-run.ps1 to have built the app.
#   powershell -File tools\splash-check.ps1 <output folder>
param([string]$Out = "$env:TEMP\quadra-splash")
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
New-Item -ItemType Directory -Force $Out | Out-Null
$exe = Join-Path $PSScriptRoot '..\src-tauri\target\debug\quadra.exe'
Get-Process quadra -ErrorAction SilentlyContinue | Where-Object { $_.Path -like '*\target\debug\*' } | Stop-Process -Force
Start-Sleep -Milliseconds 400

$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$size = 330
$left = [int]($screen.X + ($screen.Width - $size) / 2)
$top = [int]($screen.Y + ($screen.Height - $size) / 2)
function Shot($name) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($left, $top, 0, 0, $bmp.Size)
    $bmp.Save((Join-Path $Out "$name.png"))
    $g.Dispose(); $bmp.Dispose()
}

Shot 'before'
$started = Get-Date
Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe)
foreach ($ms in 500, 800, 1100, 1400, 1800, 2600) {
    $wait = $ms - ((Get-Date) - $started).TotalMilliseconds
    if ($wait -gt 0) { Start-Sleep -Milliseconds $wait }
    Shot ("t{0:d4}" -f $ms)
}
"saved to $Out"
