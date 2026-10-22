param([switch]$SkipSmoke)
$ErrorActionPreference = 'Stop'
if (-not $env:CARGO_LOG) { $env:CARGO_LOG = 'cargo::core::compiler::fingerprint=info' }

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$manifest = Join-Path $root 'apps\slint-shell\Cargo.toml'

function Invoke-Checked {
    param([string]$Command, [string[]]$Arguments)

    $ErrorActionPreference = 'Continue'
    & $Command @Arguments
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($code -ne 0) {
        throw "$Command $($Arguments -join ' ') failed with exit code $code"
    }
}

Push-Location $root
try {
    & (Join-Path $PSScriptRoot 'check-windows-syntax.ps1')
    if (-not $SkipSmoke) { & (Join-Path $PSScriptRoot 'check-windows-smoke.ps1') }
    Invoke-Checked 'cargo' @('test', '--locked', '-p', 'lhc-core', '-p', 'slint-shell', '--all-targets')
    Invoke-Checked 'cargo' @('clippy', '--locked', '-p', 'lhc-core', '-p', 'slint-shell', '--all-targets', '--no-deps', '--', '-D', 'warnings')
    Invoke-Checked 'cargo' @('test', '--release', '--locked', '-p', 'lhc-core', '-p', 'slint-shell', '--test', 'storage_override')
    Invoke-Checked 'cargo' @(
        'build', '--release', '--locked', '--manifest-path', $manifest,
        '-p', 'lhc-core', '-p', 'slint-shell',
        '--bin', 'left-hand-control', '--example', 'windows-mapper-stand'
    )


    & (Join-Path $PSScriptRoot 'package-windows.ps1')
    $metadataJson = & cargo metadata --locked --format-version 1 --no-deps
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $metadata = $metadataJson | ConvertFrom-Json
    $version = ($metadata.packages | Where-Object name -eq 'slint-shell').version
    $archive = Join-Path $metadata.target_directory "release-artifacts-windows\left-hand-control-$version-windows-x86_64.zip"
    & (Join-Path $PSScriptRoot 'check-windows-package.ps1') -Archive $archive -Desktop
} finally {
    Pop-Location
}

Write-Host 'WINDOWS AUTOMATED ACCEPTANCE PASSED' -ForegroundColor Green
