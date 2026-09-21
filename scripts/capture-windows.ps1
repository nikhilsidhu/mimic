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
    $n++
    $path = Join-Path $OutDir "window-$n.png"
    $bitmap.Save($path)
    $graphics.Dispose()
    $bitmap.Dispose()
    "$path  ${width}x${height}"
}
if ($n -eq 0) { "no visible mimic windows" }
