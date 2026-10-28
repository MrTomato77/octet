$log = Join-Path $env:TEMP 'octet-poc.log'
if (Test-Path $log) { Clear-Content $log }
$s0 = Get-NetAdapterStatistics -Name 'Wi-Fi'
$t0 = Get-Date
$env:OCTET_POC_SPEED = '1'
$app = Start-Process -FilePath 'D:\Simple IP v2\target\release\octet.exe' -ArgumentList '--admin' -PassThru
$peak = 0; $cpu = @{}; $peakIdle = 0
while (((Get-Date) - $t0).TotalSeconds -lt 300) {
  Start-Sleep -Milliseconds 500
  # Octet and the WebView2 processes it started (created after launch).
  $ps = @(Get-Process -Id $app.Id -ErrorAction SilentlyContinue) + @(Get-Process msedgewebview2 -ErrorAction SilentlyContinue | Where-Object { $_.StartTime -ge $t0 })
  $ws = ($ps | Measure-Object WorkingSet64 -Sum).Sum
  if ($ws -gt $peak) { $peak = $ws }
  foreach ($p in $ps) { try { $cpu[$p.Id] = $p.TotalProcessorTime.TotalMilliseconds } catch {} }
  if ((Test-Path $log) -and (Select-String -Path $log -Pattern 'SDK loaded' -Quiet) -and $peakIdle -eq 0) { $peakIdle = $peak }
  if ((Test-Path $log) -and (Select-String -Path $log -Pattern 'POC end' -Quiet)) { break }
}
$s1 = Get-NetAdapterStatistics -Name 'Wi-Fi'
Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
Get-Content $log
'wall {0:N1} s, processes {1}, cpu total {2:N0} ms, peak working set {3:N0} MB (at SDK load {4:N0} MB)' -f ((Get-Date) - $t0).TotalSeconds, $cpu.Count, ($cpu.Values | Measure-Object -Sum).Sum, ($peak / 1MB), ($peakIdle / 1MB)
'Wi-Fi in +{0:N1} MB, out +{1:N1} MB' -f (($s1.ReceivedBytes - $s0.ReceivedBytes) / 1e6), (($s1.SentBytes - $s0.SentBytes) / 1e6)
