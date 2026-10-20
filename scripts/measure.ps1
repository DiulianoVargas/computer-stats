<#
.SYNOPSIS
    Measures Computer Stats against the resource budgets (Constitution I).
.DESCRIPTION
    Samples the running computer-stats process once per second and reports private memory and CPU.
    Exits 1 if any budget is exceeded:
      Panel closed: <= 30 MB private, < 0.5% CPU
      Panel open:   <= 45 MB private, < 2% CPU
      Release exe:  <= 10 MB
.EXAMPLE
    .\scripts\measure.ps1 -Minutes 10
    .\scripts\measure.ps1 -Minutes 10 -PanelOpen
#>
param(
    [double]$Minutes = 10,
    [switch]$PanelOpen,
    [string]$ProcessName = 'computer-stats',
    # Measure this process instead of the first one named $ProcessName.
    [int]$ProcessId = 0,
    [string]$Exe = (Join-Path $PSScriptRoot '..\target\release\computer-stats.exe')
)

$ErrorActionPreference = 'Stop'

$maxMb = if ($PanelOpen) { 45 } else { 30 }
$maxCpu = if ($PanelOpen) { 2.0 } else { 0.5 }
$label = if ($PanelOpen) { 'Panel open' } else { 'Panel closed' }
$cores = [Environment]::ProcessorCount

$proc = if ($ProcessId) {
    Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
} else {
    Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
}
if (-not $proc) {
    Write-Error "Process '$ProcessName' is not running."
    exit 2
}

$seconds = [int]($Minutes * 60)
Write-Host "Measuring $ProcessName (pid $($proc.Id)), $label, for $seconds s..."

$samplesMb = New-Object System.Collections.Generic.List[double]
$startCpu = $proc.TotalProcessorTime
$startTime = Get-Date

for ($i = 0; $i -lt $seconds; $i++) {
    Start-Sleep -Seconds 1
    $proc.Refresh()
    if ($proc.HasExited) {
        Write-Error 'Process exited during measurement.'
        exit 2
    }
    $samplesMb.Add($proc.PrivateMemorySize64 / 1MB)
}

$elapsed = ((Get-Date) - $startTime).TotalSeconds
$cpuPct = ($proc.TotalProcessorTime - $startCpu).TotalSeconds / $elapsed / $cores * 100
$stats = $samplesMb | Measure-Object -Maximum -Average -Minimum

$fail = $false
'{0,-22} {1,8:N1} MB (min {2:N1}, mean {3:N1}; budget {4} MB)' -f 'Private memory (max)', $stats.Maximum, $stats.Minimum, $stats.Average, $maxMb | Write-Host
if ($stats.Maximum -gt $maxMb) { $fail = $true; Write-Host '  -> OVER BUDGET' -ForegroundColor Red }

'{0,-22} {1,8:N2} %  (budget < {2} %)' -f 'CPU (mean)', $cpuPct, $maxCpu | Write-Host
if ($cpuPct -ge $maxCpu) { $fail = $true; Write-Host '  -> OVER BUDGET' -ForegroundColor Red }

'{0,-22} {1,8:N1} MB (last - first sample)' -f 'Memory growth', ($samplesMb[$samplesMb.Count - 1] - $samplesMb[0]) | Write-Host

if (Test-Path $Exe) {
    $exeMb = (Get-Item $Exe).Length / 1MB
    '{0,-22} {1,8:N2} MB (budget 10 MB)' -f 'Release exe', $exeMb | Write-Host
    if ($exeMb -gt 10) { $fail = $true; Write-Host '  -> OVER BUDGET' -ForegroundColor Red }
} else {
    Write-Host "Release exe not found at $Exe (size not checked)."
}

if ($fail) { exit 1 }
Write-Host 'All budgets met.' -ForegroundColor Green
exit 0
