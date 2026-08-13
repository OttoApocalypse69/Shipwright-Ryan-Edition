param(
    [Parameter(Mandatory = $true)][string]$Version,
    [ValidateSet('stable', 'beta', 'nightly')][string]$Channel = 'stable',
    [Parameter(Mandatory = $true)][string]$ReleaseBaseUrl,
    [switch]$UnsignedLocal,
    [string]$RepositoryRoot,
    [string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) { $RepositoryRoot = (Resolve-Path (Join-Path $scriptDirectory '..\..')).Path }
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = Join-Path $RepositoryRoot 'release-assets' }
$resolvedRoot = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$targetRoot = Join-Path $resolvedRoot 'target\release'
$installer = @(Get-ChildItem -LiteralPath (Join-Path $targetRoot 'bundle\nsis') -File -Filter 'SRE_*_x64-setup.exe')
if ($installer.Count -ne 1) { throw "Expected exactly one NSIS setup executable; found $($installer.Count)." }
$launcher = Join-Path $targetRoot 'sre-launcher.exe'
if (-not (Test-Path -LiteralPath $launcher -PathType Leaf)) { throw "SRE launcher executable not found: $launcher" }
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$portableRoot = Join-Path $OutputDirectory 'portable'
if (Test-Path -LiteralPath $portableRoot) { Remove-Item -LiteralPath $portableRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path $portableRoot | Out-Null
Copy-Item -LiteralPath $installer[0].FullName -Destination (Join-Path $OutputDirectory 'SRE-Setup-x64.exe')
Copy-Item -LiteralPath $launcher -Destination (Join-Path $portableRoot 'SRE.exe')
Copy-Item -LiteralPath (Join-Path $resolvedRoot 'apps\launcher\resources\runtime') -Destination (Join-Path $portableRoot 'runtime') -Recurse
Copy-Item -LiteralPath (Join-Path $resolvedRoot 'apps\launcher\resources\trust') -Destination (Join-Path $portableRoot 'trust') -Recurse
Copy-Item -LiteralPath (Join-Path $resolvedRoot 'apps\launcher\resources\extractor') -Destination (Join-Path $portableRoot 'extractor') -Recurse
$portableZip = Join-Path $OutputDirectory 'SRE-Portable-x64.zip'
if (Test-Path -LiteralPath $portableZip) { Remove-Item -LiteralPath $portableZip -Force }
Compress-Archive -Path (Join-Path $portableRoot '*') -DestinationPath $portableZip -CompressionLevel Optimal
$assetNames = @('SRE-Setup-x64.exe', 'SRE-Portable-x64.zip')
$checksumLines = foreach ($name in $assetNames) { $hash = (Get-FileHash -LiteralPath (Join-Path $OutputDirectory $name) -Algorithm SHA256).Hash.ToLowerInvariant(); "$hash  $name" }
[System.IO.File]::WriteAllLines(
    (Join-Path $OutputDirectory 'SHA256SUMS.txt'),
    [string[]]$checksumLines,
    [System.Text.UTF8Encoding]::new($false)
)
if ($UnsignedLocal) {
    Write-Warning 'Unsigned local package: no release manifest/signature was created and these assets must not be published.'
} else {
    node (Join-Path $resolvedRoot 'tooling\scripts\create-release-manifest.mjs') $OutputDirectory $Version $Channel $ReleaseBaseUrl
    if ($LASTEXITCODE -ne 0) { throw "Release manifest signing failed with exit code $LASTEXITCODE." }
}
Remove-Item -LiteralPath $portableRoot -Recurse -Force
Get-ChildItem -LiteralPath $OutputDirectory -File | Select-Object Name, Length
