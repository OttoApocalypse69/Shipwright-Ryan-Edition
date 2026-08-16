[CmdletBinding()]
param(
    [string]$RepositoryRoot,
    [switch]$Apply,
    [switch]$IncludeRustTarget,
    [switch]$RemoveNodeModules,
    [int]$KeepRyujinxLogs = 5,
    [long]$MaxRyujinxLogBytes = 64MB,
    [int]$ActiveLogAgeMinutes = 30
)

$ErrorActionPreference = 'Stop'
$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = (Resolve-Path (Join-Path $scriptRoot '..')).Path
}
$RepositoryRoot = (Resolve-Path -LiteralPath $RepositoryRoot).Path

if ($KeepRyujinxLogs -lt 0) { throw 'KeepRyujinxLogs must be zero or greater.' }
if ($MaxRyujinxLogBytes -lt 1) { throw 'MaxRyujinxLogBytes must be greater than zero.' }
if ($ActiveLogAgeMinutes -lt 1) { throw 'ActiveLogAgeMinutes must be at least one minute.' }

function Test-SreProcess {
    $processNames = @('sre-launcher', 'sre-local-launcher', 'Ryujinx', '2ship', 'Cemu')
    return $null -ne (Get-Process -Name $processNames -ErrorAction SilentlyContinue | Select-Object -First 1)
}

function Get-DirectoryBytes {
    param([Parameter(Mandatory = $true)][string]$Path)
    $sum = (Get-ChildItem -LiteralPath $Path -File -Recurse -Force -ErrorAction SilentlyContinue |
        Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) { return [int64]0 }
    return [int64]$sum
}

function Add-Removal {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Reason,
        [Parameter(Mandatory = $true)][long]$Bytes
    )
    $script:removals += [pscustomobject]@{ Path = $Path; Reason = $Reason; Bytes = $Bytes }
}

$removals = @()
$generatedDirectories = @(
    'target-launch-performance-verify',
    'target-rom-launch-assets-verify',
    'target-rom-launch-verify',
    'build-sre-runtime',
    'build-sre-tools',
    'x64',
    'apps/web/.next'
)
if ($IncludeRustTarget) { $generatedDirectories += 'target' }
if ($RemoveNodeModules) { $generatedDirectories += 'node_modules' }

if (Test-SreProcess) { throw 'SRE or an emulator is still running. Close it before cleaning generated data.' }

foreach ($relativePath in $generatedDirectories | Select-Object -Unique) {
    $path = Join-Path $RepositoryRoot $relativePath
    if (Test-Path -LiteralPath $path -PathType Container) {
        Add-Removal -Path $path -Reason 'rebuildable generated output' -Bytes (Get-DirectoryBytes -Path $path)
    }
}

$logDirectories = @(
    (Join-Path $RepositoryRoot 'apps/launcher/resources/runtime/ryujinx-canary/publish/Logs'),
    (Join-Path $RepositoryRoot 'target/debug/runtime/ryujinx-canary/publish/Logs')
)
$logCutoff = (Get-Date).AddMinutes(-$ActiveLogAgeMinutes)
foreach ($logDirectory in $logDirectories | Select-Object -Unique) {
    if (-not (Test-Path -LiteralPath $logDirectory -PathType Container)) { continue }
    $oldLogs = @(Get-ChildItem -LiteralPath $logDirectory -File -Filter '*.log' -Force |
        Where-Object { $_.LastWriteTime -lt $logCutoff } | Sort-Object LastWriteTime -Descending)
    $retainedBytes = [int64]0
    $retainedCount = 0
    foreach ($log in $oldLogs) {
        $canRetain = $retainedCount -lt $KeepRyujinxLogs -and
            $log.Length -le $MaxRyujinxLogBytes -and
            ($retainedBytes + $log.Length) -le $MaxRyujinxLogBytes
        if ($canRetain) {
            $retainedCount++
            $retainedBytes += $log.Length
        } else {
            Add-Removal -Path $log.FullName -Reason 'stale Ryujinx log' -Bytes $log.Length
        }
    }
}

$totalBytes = ($removals | Measure-Object -Property Bytes -Sum).Sum
if ($null -eq $totalBytes) { $totalBytes = 0 }
if ($removals.Count -eq 0) { Write-Host 'No removable generated data was found.'; exit 0 }

Write-Host ("{0} item(s), {1:N2} GB reclaimable." -f $removals.Count, ($totalBytes / 1GB))
foreach ($removal in $removals) {
    Write-Host ("  {0:N1} MB  {1}  [{2}]" -f ($removal.Bytes / 1MB), $removal.Path, $removal.Reason)
}
if (-not $Apply) {
    Write-Host ''
    Write-Host 'Preview only. Re-run with -Apply to remove these generated files.'
    Write-Host 'The -IncludeRustTarget switch removes the Rust debug build and requires a rebuild.'
    exit 0
}
foreach ($removal in $removals) {
    Remove-Item -LiteralPath $removal.Path -Recurse -Force -ErrorAction Stop
}
Write-Host ("Removed generated data and reclaimed approximately {0:N2} GB." -f ($totalBytes / 1GB))
