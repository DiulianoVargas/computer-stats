<#
.SYNOPSIS
  Quickstart V5 + V6, run elevated: records cpu-temp-spike and a LibreHardwareMonitorLib reference
  side by side, at idle and under an all-core load.

.PARAMETER LhmDir
  Folder with LibreHardwareMonitorLib.dll (net472 build) and its dependencies.
#>
param(
    [Parameter(Mandatory)][string]$LhmDir,
    [int]$IdleSeconds = 60,
    [int]$LoadSeconds = 90
)

$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$exe = Join-Path $here '..\target\release\cpu-temp-spike.exe'
Start-Transcript (Join-Path $here 'run.transcript.txt') -Force | Out-Null

# --- Reference: LibreHardwareMonitorLib -------------------------------------
[AppDomain]::CurrentDomain.add_AssemblyResolve({
    param($s, $e)
    $path = Join-Path $LhmDir ((New-Object Reflection.AssemblyName $e.Name).Name + '.dll')
    if (Test-Path $path) { return [Reflection.Assembly]::LoadFrom($path) }
    return $null
})
[Reflection.Assembly]::LoadFrom((Join-Path $LhmDir 'LibreHardwareMonitorLib.dll')) | Out-Null
$computer = New-Object LibreHardwareMonitor.Hardware.Computer
$computer.IsCpuEnabled = $true
$computer.Open()
$cpu = $computer.Hardware | Where-Object { $_.HardwareType -eq 'Cpu' } | Select-Object -First 1
$cpu.Update()
$sensor = $cpu.Sensors | Where-Object { $_.SensorType -eq 'Temperature' -and $_.Name -match 'Tctl' } | Select-Object -First 1
if (-not $sensor) {
    "Available temperature sensors:"; $cpu.Sensors | Where-Object { $_.SensorType -eq 'Temperature' } | ForEach-Object { "  $($_.Name)" }
    throw 'No Tctl/Tdie sensor from LibreHardwareMonitorLib.'
}
"Reference: $($cpu.Name) / $($sensor.Name)"

function Record-Phase([string]$name, [int]$seconds) {
    $log = Join-Path $here "$name.log"
    $csv = Join-Path $here "reference-$name.csv"
    $spike = Start-Process $exe -PassThru -NoNewWindow -RedirectStandardOutput $log
    $rows = New-Object System.Collections.Generic.List[string]
    $rows.Add('"Date","Time","CPU (Tctl/Tdie) [C]"')
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) {
        # Sample mid-second so both tools land in the same HH:mm:ss bucket.
        $now = Get-Date
        Start-Sleep -Milliseconds ([Math]::Max(0, 500 - $now.Millisecond + $(if ($now.Millisecond -gt 500) { 1000 } else { 0 })))
        $cpu.Update()
        $t = Get-Date
        $rows.Add(('"{0}","{1}","{2}"' -f $t.ToString('dd.MM.yyyy'), $t.ToString('HH:mm:ss.fff'),
            ([double]$sensor.Value).ToString('0.000', [Globalization.CultureInfo]::InvariantCulture)))
    }
    Stop-Process $spike -Force
    [IO.File]::WriteAllLines($csv, $rows)
    "$name : $($rows.Count - 1) reference samples"
}

# --- V5: idle ----------------------------------------------------------------
"V5 idle: settling 10 s, recording $IdleSeconds s"
Start-Sleep 10
Record-Phase 'idle' $IdleSeconds

# --- V6: all-core load --------------------------------------------------------
$threads = [Environment]::ProcessorCount
"V6 load: $threads busy-loop workers, recording $LoadSeconds s"
$workers = 1..$threads | ForEach-Object {
    Start-Process powershell -PassThru -WindowStyle Hidden -ArgumentList '-NoProfile', '-Command', 'while ($true) { }'
}
try {
    Record-Phase 'load' $LoadSeconds
} finally {
    $workers | Stop-Process -Force -ErrorAction SilentlyContinue
    $computer.Close()
}
"Done."
Stop-Transcript | Out-Null
