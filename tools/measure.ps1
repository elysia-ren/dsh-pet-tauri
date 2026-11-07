# 采集 dsh-pet-tauri 的进程树资源：私有工作集 / 提交 / 工作集、CPU（单核 vs 整机）、GPU 引擎利用率
# 用法：powershell -ExecutionPolicy Bypass -File tools\measure.ps1 -Seconds 30
param(
  [string]$Name = "dsh-pet-tauri",
  [int]$Seconds = 30
)

$rootProc = Get-Process -Name $Name -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $rootProc) { Write-Error "process not found: $Name"; exit 1 }
$root = $rootProc.Id

$all = Get-CimInstance Win32_Process
$set = New-Object 'System.Collections.Generic.HashSet[int]'
[void]$set.Add([int]$root)
$changed = $true
while ($changed) {
  $changed = $false
  foreach ($p in $all) {
    if ($set.Contains([int]$p.ParentProcessId) -and -not $set.Contains([int]$p.ProcessId)) {
      [void]$set.Add([int]$p.ProcessId)
      $changed = $true
    }
  }
}
$ids = @($set)
Write-Host "tree pids: $($ids -join ',')"

$perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Where-Object { $ids -contains [int]$_.IDProcess }
$privWS = ($perf | Measure-Object WorkingSetPrivate -Sum).Sum / 1MB
$procs = Get-Process -Id $ids -ErrorAction SilentlyContinue
$ws = ($procs | Measure-Object WorkingSet64 -Sum).Sum / 1MB
$commit = ($procs | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB
Write-Host ("private WS: {0:N1} MB | commit: {1:N1} MB | WS sum (incl. shared): {2:N1} MB" -f $privWS, $commit, $ws)
$perf | Select-Object Name, IDProcess, @{n = 'PrivWS_MB'; e = { [math]::Round($_.WorkingSetPrivate / 1MB, 1) } }, @{n = 'WS_MB'; e = { [math]::Round($_.WorkingSet / 1MB, 1) } } | Format-Table -AutoSize | Out-String | Write-Host

$t0 = @{}
foreach ($id in $ids) { $p = Get-Process -Id $id -ErrorAction SilentlyContinue; if ($p) { $t0[$id] = $p.CPU } }
$sw = [System.Diagnostics.Stopwatch]::StartNew()
Start-Sleep -Seconds $Seconds
$sw.Stop()
$el = $sw.Elapsed.TotalSeconds

$total = 0.0
$rows = @()
foreach ($id in $ids) {
  $p = Get-Process -Id $id -ErrorAction SilentlyContinue
  if ($p) {
    $d = $p.CPU - $t0[$id]
    $total += $d
    $rows += [pscustomobject]@{ PID = $id; Name = $p.ProcessName; CPUsec = [math]::Round($d, 2); PctOneCore = [math]::Round($d / $el * 100, 2) }
  }
}
$rows | Format-Table -AutoSize | Out-String | Write-Host

$n = $env:NUMBER_OF_PROCESSORS
Write-Host ("TOTAL CPU: {0:N2}% 单核 = {1:N2}% 整机（{2} 线程）" -f ($total / $el * 100), ($total / $el * 100 / $n), $n)

try {
  $gc = Get-Counter '\GPU Engine(*)\Utilization Percentage' -ErrorAction Stop
  $gpu = ($gc.CounterSamples | Where-Object { $ids -contains [int]([regex]::Match($_.InstanceName, 'pid_(\d+)').Groups[1].Value) } | Measure-Object CookedValue -Sum).Sum
  Write-Host ("GPU utilization (sum of engines): {0:N2}%" -f $gpu)
} catch {
  Write-Host "GPU counter unavailable: $($_.Exception.Message)"
}
