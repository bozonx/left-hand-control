param(
    [Parameter(Mandatory = $true)][string]$Archive, [switch]$Desktop,
    [ValidateRange(100, 10000)][int]$StressCycles = 100,
    [ValidateRange(0, 86400)][int]$StressSeconds = 0
)
$ErrorActionPreference = 'Stop'
$archivePath = (Resolve-Path $Archive).Path
$checksumPath = Join-Path (Split-Path $archivePath) 'WINDOWS-SHA256SUMS'
$expected = (Get-Content $checksumPath -Raw).Trim()
$actual = (Get-FileHash -Algorithm SHA256 $archivePath).Hash.ToLowerInvariant()
if ($expected -ne "$actual  $(Split-Path $archivePath -Leaf)") { throw 'ZIP checksum mismatch' }
$extracted = Join-Path ([IO.Path]::GetTempPath()) ("lhc-package-" + [guid]::NewGuid().ToString())
try {
    Expand-Archive -LiteralPath $archivePath -DestinationPath $extracted
    $executables = @(Get-ChildItem $extracted -Recurse -File -Filter left-hand-control.exe)
    if ($executables.Count -ne 1) { throw 'Package must contain exactly one executable' }
    $exe = $executables[0].FullName
    foreach ($file in @('LICENSE', 'README.md', 'features.md')) {
        if (-not (Test-Path (Join-Path $executables[0].DirectoryName $file))) { throw "Package missing $file" }
    }
    & $exe --version
    if ($LASTEXITCODE -ne 0) { throw 'Packaged --version failed' }
    & $exe --help
    if ($LASTEXITCODE -ne 0) { throw 'Packaged --help failed' }
    if ($Desktop) {
        & (Join-Path $PSScriptRoot 'check-windows-lifecycle.ps1') -Executable $exe -StressCycles $StressCycles -StressSeconds $StressSeconds
    }
    Write-Host "WINDOWS PACKAGE PASSED: $(Split-Path $archivePath -Leaf) SHA256=$actual desktop=$Desktop"
} finally {
    if (Test-Path $extracted) { Remove-Item $extracted -Recurse -Force }
}
