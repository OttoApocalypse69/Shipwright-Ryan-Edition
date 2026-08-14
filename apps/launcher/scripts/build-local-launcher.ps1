[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$manifestPath = Join-Path $repositoryRoot 'apps\launcher\src-tauri\Cargo.toml'
$outputPath = Join-Path $repositoryRoot 'target\debug\sre-local-launcher.exe'
$visibleLauncherPath = Join-Path $repositoryRoot 'START SRE.exe'

& cargo build --manifest-path $manifestPath --bin sre-local-launcher --locked
if ($LASTEXITCODE -ne 0) {
    throw "Could not build the local SRE launcher (exit code $LASTEXITCODE)."
}

Copy-Item -LiteralPath $outputPath -Destination $visibleLauncherPath -Force
Write-Output "Built $visibleLauncherPath"
