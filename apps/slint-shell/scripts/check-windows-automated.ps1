$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$manifest = Join-Path $root 'apps\slint-shell\Cargo.toml'
$interaction = Join-Path $root 'target\release\examples\interactions.exe'

function Invoke-Checked {
    param([string]$Command, [string[]]$Arguments)

    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Command $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

Push-Location $root
try {
    Invoke-Checked 'cargo' @('test', '--locked', '--manifest-path', $manifest, '--lib', '--bins')
    Invoke-Checked 'cargo' @(
        'build', '--release', '--locked', '--manifest-path', $manifest,
        '--bin', 'slint-shell', '--example', 'interactions'
    )

    $env:SLINT_BACKEND = 'winit-software'
    Invoke-Checked $interaction @()
    Remove-Item Env:SLINT_BACKEND -ErrorAction SilentlyContinue

    & (Join-Path $PSScriptRoot 'check-windows-lifecycle.ps1')
} finally {
    Remove-Item Env:SLINT_BACKEND -ErrorAction SilentlyContinue
    Pop-Location
}

Write-Host 'WINDOWS AUTOMATED ACCEPTANCE PASSED' -ForegroundColor Green
