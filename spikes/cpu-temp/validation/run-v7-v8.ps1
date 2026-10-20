<#
.SYNOPSIS
  Quickstart V7 (5-minute stability) + V8 (memory), run elevated.
#>
param([int]$Seconds = 300)

$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$exe = Join-Path $here '..\target\release\cpu-temp-spike.exe'
$log = Join-Path $here 'v7.log'
$mem = Join-Path $here 'v8-memory.csv'

$spike = Start-Process $exe -PassThru -NoNewWindow -RedirectStandardOutput $log
$samples = New-Object System.Collections.Generic.List[string]
$samples.Add('"Elapsed_s","PrivateMB","WorkingSetMB"')
$start = Get-Date
while (((Get-Date) - $start).TotalSeconds -lt $Seconds) {
    Start-Sleep 10
    $spike.Refresh()
    if ($spike.HasExited) { break }
    $inv = [Globalization.CultureInfo]::InvariantCulture
    $samples.Add([string]::Format($inv, '{0},{1:0.00},{2:0.00}', [int]((Get-Date) - $start).TotalSeconds,
        ($spike.PrivateMemorySize64 / 1MB), ($spike.WorkingSet64 / 1MB)))
}
$exitedEarly = $spike.HasExited
if (-not $exitedEarly) { Stop-Process $spike -Force }
[IO.File]::WriteAllLines($mem, $samples)
"exitedEarly=$exitedEarly" | Set-Content (Join-Path $here 'v7-status.txt')
