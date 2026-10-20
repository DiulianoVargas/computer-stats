<#
.SYNOPSIS
    Builds the release exe and packages it as an installer and a portable zip.
.DESCRIPTION
    1. cargo build --release (static CRT, icon and version embedded).
    2. Downloads PawnIO.Modules $ModulesVersion and checks AMDFamily17.bin's SHA-256 against the
       copy validated in spike 001.
    3. Collects third-party notices: Slint, PawnIO.Modules (LGPL-2.1 + source link), and the
       license files of every Rust crate in the build.
    4. Compiles installer/computer-stats.iss with Inno Setup into dist\.
    5. Writes dist\ComputerStats-<version>-portable.zip (exe, module, notices).
.EXAMPLE
    .\scripts\package.ps1
#>
param(
    [string]$ModulesVersion = '0.2.11',
    [string]$ModuleSha256 = 'DAE74615761B78BDF064DFB3E136252DDCC6FC727D88F14738D0E5800D427A91',
    [string]$Iscc = (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe')
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$work = Join-Path $root 'target\package'
$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $work, $dist, "$work\module", "$work\licenses" | Out-Null

Write-Host "== Computer Stats $version"

# 1. Build
cargo build --release
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
$exe = Join-Path $root 'target\release\computer-stats.exe'
$exeMb = (Get-Item $exe).Length / 1MB
Write-Host ("exe: {0:N2} MiB" -f $exeMb)
if ($exeMb -gt 10) { throw 'exe exceeds the 10 MB budget (Constitution I)' }

# 2. CPU Temperature module
$zip = Join-Path $work "PawnIO.Modules-$ModulesVersion.zip"
if (-not (Test-Path $zip)) {
    $tag = $ModulesVersion -replace '\.', '_'
    $url = "https://github.com/namazso/PawnIO.Modules/releases/download/$ModulesVersion/release_$tag.zip"
    Write-Host "downloading $url"
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
}
$extract = Join-Path $work 'modules-extract'
Remove-Item $extract -Recurse -Force -ErrorAction SilentlyContinue
Expand-Archive $zip $extract
$module = Get-ChildItem $extract -Recurse -Filter AMDFamily17.bin | Select-Object -First 1
if (-not $module) { throw 'AMDFamily17.bin not found in the PawnIO.Modules release' }
$hash = (Get-FileHash $module.FullName -Algorithm SHA256).Hash
if ($hash -ne $ModuleSha256) { throw "AMDFamily17.bin hash $hash differs from the validated $ModuleSha256" }
Copy-Item $module.FullName "$work\module\AMDFamily17.bin" -Force

# 3. Third-party notices
$licenseUrl = "https://raw.githubusercontent.com/namazso/PawnIO.Modules/$ModulesVersion/COPYING"
Invoke-WebRequest -Uri $licenseUrl -OutFile "$work\licenses\PawnIO.Modules-LGPL-2.1.txt" -UseBasicParsing

$meta = cargo metadata --format-version 1 --filter-platform x86_64-pc-windows-msvc | ConvertFrom-Json
$ids = @{}
foreach ($node in $meta.resolve.nodes) { $ids[$node.id] = $node }
# Packages reachable from computer-stats through normal (runtime) dependencies only.
$rootId = ($meta.packages | Where-Object name -eq 'computer-stats').id
$runtime = @{}
$queue = [System.Collections.Generic.Queue[string]]::new(); $queue.Enqueue($rootId)
while ($queue.Count -gt 0) {
    $id = $queue.Dequeue()
    foreach ($dep in $ids[$id].deps) {
        $normal = $dep.dep_kinds | Where-Object { $null -eq $_.kind }
        if ($normal -and -not $runtime.ContainsKey($dep.pkg)) { $runtime[$dep.pkg] = $true; $queue.Enqueue($dep.pkg) }
    }
}
$crates = $meta.packages | Where-Object { $runtime.ContainsKey($_.id) } | Sort-Object name, version
$sb = [System.Text.StringBuilder]::new()
[void]$sb.AppendLine('Rust crates compiled into computer-stats.exe, with their license files.')
[void]$sb.AppendLine()
foreach ($c in $crates) {
    [void]$sb.AppendLine(('=' * 78))
    [void]$sb.AppendLine("$($c.name) $($c.version) - $($c.license)")
    if ($c.repository) { [void]$sb.AppendLine($c.repository) }
    [void]$sb.AppendLine(('=' * 78))
    $dir = Split-Path $c.manifest_path -Parent
    $files = Get-ChildItem $dir -File | Where-Object { $_.Name -match '^(LICEN[CS]E|COPYING|NOTICE)' }
    foreach ($f in $files) { [void]$sb.AppendLine("--- $($f.Name)"); [void]$sb.AppendLine((Get-Content $f.FullName -Raw)) }
    if (-not $files) { [void]$sb.AppendLine("(no license file shipped in the crate; license: $($c.license))") }
    [void]$sb.AppendLine()
}
Set-Content -Path "$work\licenses\rust-crates.txt" -Value $sb.ToString() -Encoding utf8
Write-Host "notices: $($crates.Count) Rust crates"

@"
Computer Stats $version
https://github.com/DiulianoVargas/computer-stats

This application includes third-party software:

Slint (https://slint.dev)
  Used under the Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0:
  https://slint.dev/terms-and-conditions#royalty-free

PawnIO.Modules $ModulesVersion - AMDFamily17.bin (used for CPU Temperature)
  Copyright (c) namazso. Licensed under the GNU Lesser General Public License v2.1
  (licenses\PawnIO.Modules-LGPL-2.1.txt). Unmodified; source code:
  https://github.com/namazso/PawnIO.Modules/tree/$ModulesVersion
  The PawnIO driver is not included; install it from https://pawnio.eu to see CPU Temperature.

Rust crates
  See licenses\rust-crates.txt for each crate's name, version, license and license text.
"@ | Set-Content -Path "$work\THIRD-PARTY-NOTICES.txt" -Encoding utf8

# 4. Installer
if (-not (Test-Path $Iscc)) { throw "Inno Setup not found at $Iscc (winget install JRSoftware.InnoSetup)" }
& $Iscc "/DAppVersion=$version" "/DExePath=$exe" "/DPackageDir=$work" /Q (Join-Path $root 'installer\computer-stats.iss')
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup failed' }
Move-Item "$work\ComputerStats-Setup-$version.exe" "$dist\ComputerStats-Setup-$version.exe" -Force

# 5. Portable zip
$portable = Join-Path $work 'portable\Computer Stats'
Remove-Item (Split-Path $portable) -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$portable\licenses" | Out-Null
Copy-Item $exe, "$work\module\AMDFamily17.bin", "$work\THIRD-PARTY-NOTICES.txt" $portable
Copy-Item "$work\licenses\*" "$portable\licenses"
$zipOut = "$dist\ComputerStats-$version-portable.zip"
Remove-Item $zipOut -ErrorAction SilentlyContinue
Compress-Archive -Path $portable -DestinationPath $zipOut

Get-ChildItem $dist | ForEach-Object { Write-Host ("{0}  {1:N2} MiB" -f $_.Name, ($_.Length / 1MB)) }
