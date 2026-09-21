# Saves a screenshot of every visible mimic window, for checking UI work without being at
# the screen. Run a second copy of mimic first to bring up the manager: the running one
# takes over and shows it.
#
#   powershell -File scripts/capture-windows.ps1 [-OutDir <folder>]
param([string]$OutDir = "$env:TEMP\mimic-shots")

New-Item -ItemType Directory -Force $OutDir | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class MimicWin {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@
# Without this the coordinates are scaled and the capture is cropped on high-DPI screens.
[MimicWin]::SetProcessDPIAware() | Out-Null

# Every running mimic: an installed copy and a development one can run side by side.
$apps = @(Get-Process mimic -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
if (-not $apps) { Write-Error "mimic is not running"; exit 1 }

$script:found = @()
[MimicWin]::EnumWindows({
    param($h, $l)
    $owner = 0
    [MimicWin]::GetWindowThreadProcessId($h, [ref]$owner) | Out-Null
    if ($apps -contains $owner -and [MimicWin]::IsWindowVisible($h)) {
        $r = New-Object MimicWin+RECT
        [MimicWin]::GetWindowRect($h, [ref]$r) | Out-Null
        if (($r.R - $r.L) -gt 100) { $script:found += , @($h, $r) }
    }
    $true
}, [IntPtr]::Zero) | Out-Null

$n = 0
foreach ($window in $script:found) {
    $h, $r = $window
    $width = $r.R - $r.L
    $height = $r.B - $r.T
    $bitmap = New-Object System.Drawing.Bitmap $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    # The window draws itself into the bitmap, so it does not matter what covers it on
    # screen. Copying from the screen instead captured whatever was in front. Flag 2 is
    # PW_RENDERFULLCONTENT, which web view content needs.
    $hdc = $graphics.GetHdc()
    [MimicWin]::PrintWindow($h, $hdc, 2) | Out-Null
    $graphics.ReleaseHdc($hdc)
    $graphics.Dispose()
    # A window's rectangle includes the invisible border Windows gives it for resizing, about 8px
    # on the left, right and bottom, and a hairline frame. The picture is trimmed to the client
    # area, which is the page and nothing else.
    $client = New-Object MimicWin+RECT
    $origin = New-Object MimicWin+POINT
    if ([MimicWin]::GetClientRect($h, [ref]$client) -and [MimicWin]::ClientToScreen($h, [ref]$origin) -and $client.R -gt 0) {
        $visible = New-Object System.Drawing.Rectangle ($origin.X - $r.L), ($origin.Y - $r.T), $client.R, $client.B
        $visible.Intersect((New-Object System.Drawing.Rectangle 0, 0, $width, $height))
        $trimmed = $bitmap.Clone($visible, $bitmap.PixelFormat)
        $bitmap.Dispose()
        $bitmap = $trimmed
    }
    $n++
    $path = Join-Path $OutDir "window-$n.png"
    $bitmap.Save($path)
    "$path  $($bitmap.Width)x$($bitmap.Height)"
    $bitmap.Dispose()
}
if ($n -eq 0) { "no visible mimic windows" }
