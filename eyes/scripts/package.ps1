param([switch]$ResetAssets)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    cargo build --release --bin eyes --example controller
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
    $bundleRoot = Join-Path $projectRoot 'dist\eyes'
    New-Item -ItemType Directory -Force -Path $bundleRoot | Out-Null
    Copy-Item -LiteralPath (Join-Path $projectRoot 'target\release\eyes.exe') -Destination $bundleRoot -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'target\release\examples\controller.exe') -Destination $bundleRoot -Force
    # 默认保留用户画稿；修复素材时显式重置并备份旧文件，避免损坏副本被继续沿用。
    $assetSource = Join-Path $projectRoot 'assets'
    foreach ($assetDestination in @((Join-Path $bundleRoot 'assets'), (Join-Path $projectRoot 'target\release\assets'))) {
        foreach ($sourceFile in Get-ChildItem -LiteralPath $assetSource -File -Recurse) {
            $relativeName = [IO.Path]::GetRelativePath($assetSource, $sourceFile.FullName)
            $destinationFile = Join-Path $assetDestination $relativeName
            New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destinationFile) | Out-Null
            if (Test-Path -LiteralPath $destinationFile) {
                if (-not $ResetAssets -and $relativeName -notin @('eyes.wgsl', 'README.md')) { continue }
                if ((Get-FileHash -LiteralPath $destinationFile).Hash -eq (Get-FileHash -LiteralPath $sourceFile.FullName).Hash) { continue }
                if ($ResetAssets -or $relativeName -eq 'eyes.wgsl') { Copy-Item -LiteralPath $destinationFile -Destination ($destinationFile + '.backup-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff')) }
            }
            Copy-Item -LiteralPath $sourceFile.FullName -Destination $destinationFile -Force
        }
    }
    Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination $bundleRoot -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'q-eyes-preview.png') -Destination $bundleRoot -Force
    New-Item -ItemType Directory -Force -Path (Join-Path $bundleRoot 'examples'), (Join-Path $bundleRoot 'proto') | Out-Null
    Copy-Item -LiteralPath (Join-Path $projectRoot 'examples\simulator.json') -Destination (Join-Path $bundleRoot 'examples') -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'proto\eyes.proto') -Destination (Join-Path $bundleRoot 'proto') -Force
    Write-Host "Portable app: $bundleRoot"
} finally {
    Pop-Location
}
