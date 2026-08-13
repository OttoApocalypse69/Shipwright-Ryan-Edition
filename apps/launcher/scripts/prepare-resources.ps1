param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path,
    [string]$ToolsBuild = (Join-Path $RepositoryRoot 'build-ftep-tools'),
    [string]$RuntimeBuild = (Join-Path $RepositoryRoot 'build-ftep-runtime'),
    [string]$RuntimeOutput = (Join-Path $RepositoryRoot 'x64\Release')
)

$ErrorActionPreference = 'Stop'

function Resolve-UniqueArtifact {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Name
    )

    if (-not (Test-Path -LiteralPath $Root -PathType Container)) {
        throw "Build directory does not exist: $Root"
    }
    $matches = @(Get-ChildItem -LiteralPath $Root -Recurse -File -Filter $Name)
    if ($matches.Count -eq 0) {
        throw "Required artifact '$Name' was not found below $Root"
    }
    $preferred = @($matches | Where-Object { $_.FullName -notmatch '\\CMakeFiles\\|\\vcpkg_installed\\' })
    if ($preferred.Count -eq 1) {
        return $preferred[0].FullName
    }
    if ($matches.Count -eq 1) {
        return $matches[0].FullName
    }
    throw "Artifact '$Name' was ambiguous below ${Root}: $($matches.FullName -join ', ')"
}

$runtimeTarget = Join-Path $RepositoryRoot 'apps\launcher\resources\runtime'
$extractorTarget = Join-Path $RepositoryRoot 'apps\launcher\resources\extractor'
New-Item -ItemType Directory -Force -Path $runtimeTarget, $extractorTarget | Out-Null

$extractor = Resolve-UniqueArtifact -Root $ToolsBuild -Name 'soh-torch.exe'
$runtime = Resolve-UniqueArtifact -Root $RuntimeOutput -Name 'soh.exe'
$runtimeArchive = Resolve-UniqueArtifact -Root $RuntimeBuild -Name 'soh.o2r'

Copy-Item -LiteralPath $extractor -Destination (Join-Path $extractorTarget 'soh-torch.exe') -Force
Copy-Item -LiteralPath $runtime -Destination (Join-Path $runtimeTarget 'soh.exe') -Force
Copy-Item -LiteralPath $runtimeArchive -Destination (Join-Path $runtimeTarget 'soh.o2r') -Force

foreach ($artifact in @($extractor, $runtime)) {
    $artifactDirectory = Split-Path -Parent $artifact
    Get-ChildItem -LiteralPath $artifactDirectory -File -Filter '*.dll' | ForEach-Object {
        $target = if ($artifact -eq $extractor) { $extractorTarget } else { $runtimeTarget }
        Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $target $_.Name) -Force
    }
}

Write-Host "Prepared FTEP launcher resources."
Write-Host "  Extractor: $extractor"
Write-Host "  Runtime:   $runtime"
Write-Host "  Archive:   $runtimeArchive"
