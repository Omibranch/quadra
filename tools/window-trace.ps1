# Starts the debug app and lists its visible top-level windows over the first seconds:
# which window is on screen when, how big, and whether it is layered/topmost.
Add-Type @'
using System; using System.Collections.Generic; using System.Runtime.InteropServices; using System.Text;
public static class Win {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int i);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  public static List<string> Visible(uint pid) {
    var o = new List<string>();
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid && IsWindowVisible(h)) {
        RECT r; GetWindowRect(h, out r);
        var t = new StringBuilder(80); GetWindowText(h, t, 80);
        var c = new StringBuilder(80); GetClassName(h, c, 80);
        int ex = GetWindowLong(h, -20);
        o.Add(String.Format("{0}x{1} at {2},{3} '{4}' [{5}]{6}{7}", r.R - r.L, r.B - r.T, r.L, r.T, t, c,
          (ex & 0x8) != 0 ? " topmost" : "", (ex & 0x80000) != 0 ? " layered" : ""));
      }
      return true;
    }, IntPtr.Zero);
    return o;
  }
}
'@
$exe = Join-Path $PSScriptRoot '..\src-tauri\target\debug\quadra.exe'
Get-Process quadra -ErrorAction SilentlyContinue | Where-Object { $_.Path -like '*debug*' } | Stop-Process -Force
Start-Sleep -Milliseconds 400
$started = Get-Date
$p = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru
foreach ($ms in 300, 600, 900, 1200, 1600, 2000, 2400, 2800, 3300, 4000, 5000) {
    $wait = $ms - ((Get-Date) - $started).TotalMilliseconds
    if ($wait -gt 0) { Start-Sleep -Milliseconds $wait }
    $w = [Win]::Visible([uint32]$p.Id)
    "{0,5} ms: {1}" -f $ms, $(if ($w.Count) { $w -join ' | ' } else { '(nothing visible)' })
}
