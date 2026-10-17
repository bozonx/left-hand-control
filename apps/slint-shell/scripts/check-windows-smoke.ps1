# Fast desktop smoke suite. Run as the logged-in Windows test user, not via SSH.
param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
if ([System.Diagnostics.Process]::GetCurrentProcess().SessionId -eq 0) {
    throw 'Run smoke in the interactive desktop session.'
}
$principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run smoke without elevation so it exercises ordinary application permissions.'
}
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$output = Join-Path $root 'windows-runtime\smoke'
$exe = Join-Path $root 'target\debug\left-hand-control.exe'
if (Get-Process -Name 'left-hand-control' -ErrorAction SilentlyContinue) {
    throw 'Close the running application before smoke: Windows IPC shares an authentication token.'
}
$server = $null
$variables = @('LHC_DEV_DIR', 'LHC_PORT', 'LHC_HOTKEYS', 'LHC_POPUPS', 'LHC_METRICS', 'SLINT_BACKEND')
$saved = @{}
foreach ($name in $variables) { $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }

function Invoke-Checked([string]$Command, [string[]]$Arguments) {
    Get-Command $Command -ErrorAction Stop | Out-Null
    # Windows PowerShell 5.1 treats native stderr as errors; Cargo uses it for progress.
    $ErrorActionPreference = 'Continue'
    & $Command @Arguments
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($code -ne 0) { throw "$Command $($Arguments -join ' ') exited with $code" }
}

function Wait-Until([string]$Description, [scriptblock]$Condition) {
    $deadline = (Get-Date).AddSeconds(60)
    do {
        if (& $Condition) { return }
        if ($server -and $server.HasExited) { throw "Server exited while waiting for $Description" }
        Start-Sleep -Milliseconds 100
    } while ((Get-Date) -lt $deadline)
    throw "Timed out waiting for $Description"
}

New-Item -ItemType Directory -Force -Path $output | Out-Null
$env:LHC_DEV_DIR = Join-Path $output ([guid]::NewGuid().ToString())
$env:LHC_HOTKEYS = 'off'
$env:LHC_POPUPS = 'winit'
$env:SLINT_BACKEND = 'winit-software'
# Select an unused loopback port so another running instance cannot satisfy ping.
$listener = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Loopback, 0)
$listener.Start()
$env:LHC_PORT = [string]$listener.LocalEndpoint.Port
$listener.Stop()
$env:LHC_METRICS = Join-Path $output 'metrics.csv'
Remove-Item $env:LHC_METRICS -ErrorAction SilentlyContinue
Push-Location $root
try {
    if (-not $SkipBuild) {
        Invoke-Checked 'cargo' @('test', '--locked', '-p', 'lhc-core', '--lib')
        Invoke-Checked 'cargo' @('build', '--locked', '-p', 'slint-shell', '--bin', 'left-hand-control', '--example', 'editor', '--example', 'interactions')
    }
    Invoke-Checked (Join-Path $root 'target\debug\examples\editor.exe') @('--smoke')
    Invoke-Checked (Join-Path $root 'target\debug\examples\interactions.exe') @()
    Invoke-Checked $exe @('--version')
    Invoke-Checked $exe @('--help')
    $server = Start-Process $exe -WorkingDirectory $root -PassThru `
        -RedirectStandardOutput (Join-Path $output 'stdout.log') `
        -RedirectStandardError (Join-Path $output 'stderr.log')
    Wait-Until 'IPC readiness' {
        $ErrorActionPreference = 'Continue'
        & $exe ping 2>$null | Out-Null
        return $LASTEXITCODE -eq 0
    }
    # Second launch must delegate to the existing process and terminate.
    $second = Start-Process $exe -WorkingDirectory $root -PassThru
    if (-not $second.WaitForExit(20000)) {
        Stop-Process -Id $second.Id -Force -ErrorAction SilentlyContinue
        throw 'Second instance did not delegate'
    }
    if ($second.ExitCode -ne 0) { throw "Second instance exited with $($second.ExitCode)" }
    Invoke-Checked $exe @('hide')
    foreach ($popup in @('emoji', 'quick')) {
        for ($cycle = 1; $cycle -le 3; $cycle++) {
            $before = 0
            if (Test-Path $env:LHC_METRICS) {
                $before = @(Import-Csv $env:LHC_METRICS | Where-Object { $_.event -eq 'hidden' -and $_.window -eq $popup }).Count
            }
            Invoke-Checked $exe @('show', $popup)
            # queued alone is insufficient: wait until the UI processed the trigger.
            Wait-Until "$popup shown" {
                $rows = @(Import-Csv $env:LHC_METRICS)
                @($rows | Where-Object { $_.event -eq 't2_shown' -and $_.window -eq $popup }).Count -ge $cycle
            }
            Invoke-Checked $exe @('hide')
            Wait-Until "$popup hide" {
                @(Import-Csv $env:LHC_METRICS | Where-Object { $_.event -eq 'hidden' -and $_.window -eq $popup }).Count -gt $before
            }
        }
    }
    Invoke-Checked $exe @('show', 'settings')
    Invoke-Checked $exe @('preferences', 'dark', 'ru')
    Invoke-Checked $exe @('preferences', 'light', 'en')
    Invoke-Checked $exe @('quit')
    if (-not $server.WaitForExit(20000)) { throw 'Server did not exit after quit' }
    if ($server.ExitCode -ne 0) { throw "Server exited with $($server.ExitCode)" }
    if (Get-NetTCPConnection -LocalPort ([int]$env:LHC_PORT) -State Listen -ErrorAction SilentlyContinue) {
        throw 'IPC listener leaked after quit'
    }
    if ((Get-Content (Join-Path $output 'stderr.log') -Raw) -match 'panicked at') {
        throw 'Server log contains a panic'
    }
    Write-Host 'WINDOWS SMOKE PASSED: editor, interactions, CLI, second instance, popups, preferences, quit'
} finally {
    if ($server -and -not $server.HasExited) { Stop-Process -Id $server.Id -Force }
    foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
    Pop-Location
}
