param([switch]$SkipSmoke)
$ErrorActionPreference = 'Stop'

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


    & (Join-Path $PSScriptRoot 'check-windows-lifecycle.ps1')
} finally {
    Pop-Location
}

Write-Host 'WINDOWS AUTOMATED ACCEPTANCE PASSED' -ForegroundColor Green
