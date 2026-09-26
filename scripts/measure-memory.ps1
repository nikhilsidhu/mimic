# Prints what mimic costs while it runs: its own process, and the WebView2 processes that draw
# its windows. For comparing before and after a change; run it with mimic idle in the tray.
param([string]$Which = "installed")
$path = switch ($Which) { "dev" { "*target\debug*" } "release" { "*target\release*" } default { "*AppData\Local\mimic*" } }
$app = Get-Process mimic -ErrorAction SilentlyContinue | Where-Object { $_.Path -like $path } | Select-Object -First 1
if (-not $app) { "mimic ($Which) is not running"; exit 1 }
$mb = { param($bytes) [math]::Round($bytes / 1MB) }
"mimic: $(& $mb $app.WorkingSet64) MB working set, $(& $mb $app.PrivateMemorySize64) MB private"
# WebView2 helpers name the app they serve on their command line.
$helpers = Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -like "*mimic*" }
$total = 0
foreach ($h in $helpers) {
    $p = Get-Process -Id $h.ProcessId -ErrorAction SilentlyContinue
    if (-not $p) { continue }
    $kind = if ($h.CommandLine -match "--type=(\w+)") { $Matches[1] } else { "browser" }
    "  webview $kind`: $(& $mb $p.WorkingSet64) MB"
    $total += $p.WorkingSet64
}
"webview: $(@($helpers).Count) processes, $(& $mb $total) MB"
"total: $(& $mb ($total + $app.WorkingSet64)) MB"
