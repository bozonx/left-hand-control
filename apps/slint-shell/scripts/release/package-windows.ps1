# Build the portable Windows x64 distribution. No user configuration is bundled.
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Run on native Windows x64.' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
Push-Location $root
$staging = $null

function Invoke-Cargo {
    param([string[]]$Arguments)
    # Windows PowerShell 5.1 turns redirected native stderr into error records.
    $ErrorActionPreference = 'Continue'
    $output = & cargo @Arguments
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($code -ne 0) { throw "cargo $($Arguments -join ' ') failed with exit code $code" }
    return $output
}
try {
    $metadataJson = Invoke-Cargo -Arguments @('metadata', '--locked', '--format-version', '1', '--no-deps')
    $metadata = $metadataJson | ConvertFrom-Json
    $version = ($metadata.packages | Where-Object name -eq 'slint-shell').version
    $hostInfo = & rustc -vV
    if ($LASTEXITCODE -ne 0 -or $hostInfo -notcontains 'host: x86_64-pc-windows-msvc') { throw 'Windows x64 MSVC host required' }
    # Match the e2e build's dev-dependency feature union to reuse its tested binary.
    Invoke-Cargo -Arguments @('build', '--release', '--locked', '-p', 'lhc-core', '-p', 'slint-shell', '--bin', 'left-hand-control', '--example', 'windows-mapper-stand')
    $binary = Join-Path $metadata.target_directory 'release\left-hand-control.exe'
    $out = Join-Path $metadata.target_directory 'release-artifacts-windows'
    New-Item -ItemType Directory -Force $out | Out-Null
    $name = "left-hand-control-$version-windows-x86_64"
    $staging = Join-Path $out ([guid]::NewGuid().ToString())
    $bundle = Join-Path $staging $name
    New-Item -ItemType Directory -Force $bundle | Out-Null
    Copy-Item $binary (Join-Path $bundle 'left-hand-control.exe')
    Copy-Item (Join-Path $root 'LICENSE') $bundle
    Copy-Item (Join-Path $root 'docs\install-windows.md') (Join-Path $bundle 'README.md')
    Copy-Item (Join-Path $root 'docs\features.md') $bundle
    $archive = Join-Path $out "$name.zip"
    Compress-Archive -Path $bundle -DestinationPath $archive -Force
    $hash = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $out 'WINDOWS-SHA256SUMS'), "$hash  $name.zip`n", [Text.Encoding]::ASCII)
    Write-Host "Portable Windows package: $archive"
} finally {
    if ($staging -and (Test-Path $staging)) { Remove-Item $staging -Recurse -Force }
    Pop-Location
}
