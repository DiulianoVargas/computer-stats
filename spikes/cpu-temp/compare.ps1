<#
.SYNOPSIS
  Compares a cpu-temp-spike log with an HWiNFO sensor CSV log (quickstart V5/V6, SC-002).

.EXAMPLE
  .\compare.ps1 -Spike idle.log -Hwinfo hwinfo-idle.csv
#>
param(
    [Parameter(Mandatory)][string]$Spike,
    [Parameter(Mandatory)][string]$Hwinfo,
    [double]$Tolerance = 2.0
)

# Spike lines: "HH:MM:SS  54.3 °C" (lines with — are skipped).
$spikeByTime = @{}
foreach ($line in Get-Content -Encoding UTF8 $Spike) {
    if ($line -match '^(\d{2}:\d{2}:\d{2})\s+(-?\d+\.\d)\s') {
        $spikeByTime[$Matches[1]] = [double]::Parse($Matches[2], [Globalization.CultureInfo]::InvariantCulture)
    }
}

# HWiNFO CSV: a "Time" column (HH:mm:ss[.fff]) and a "CPU (Tctl/Tdie)" column.
$rows = Import-Csv $Hwinfo
$columns = $rows[0].PSObject.Properties.Name
$timeCol = $columns | Where-Object { $_ -eq 'Time' } | Select-Object -First 1
$tempCol = $columns | Where-Object { $_ -match 'Tctl/Tdie' } | Select-Object -First 1
if (-not $timeCol -or -not $tempCol) {
    Write-Error "HWiNFO CSV must have 'Time' and 'CPU (Tctl/Tdie)' columns. Found: $($columns -join ', ')"
    exit 1
}

$deviations = foreach ($row in $rows) {
    $time = ($row.$timeCol -split '\.')[0]
    if (-not $spikeByTime.ContainsKey($time)) { continue }
    $reference = $row.$tempCol -replace ',', '.'
    $ref = 0.0
    if (-not [double]::TryParse($reference, [Globalization.NumberStyles]::Float, [Globalization.CultureInfo]::InvariantCulture, [ref]$ref)) { continue }
    [math]::Abs($spikeByTime[$time] - $ref)
}

if (-not $deviations) {
    Write-Error 'No paired samples: check that both logs cover the same time window.'
    exit 1
}

$stats = $deviations | Measure-Object -Maximum -Average
$verdict = if ($stats.Maximum -le $Tolerance) { 'PASS' } else { 'FAIL' }
'Paired samples : {0}' -f $stats.Count
'Max deviation  : {0:N2} °C' -f $stats.Maximum
'Mean deviation : {0:N2} °C' -f $stats.Average
'Result         : {0} (tolerance ±{1} °C)' -f $verdict, $Tolerance
if ($verdict -eq 'FAIL') { exit 1 }
