$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$exe = Join-Path $root 'prototypes\slint-shell\target\release\slint-shell.exe'
$output = Join-Path $root 'windows-runtime\lifecycle'

if (-not (Test-Path $exe)) {
    throw "Release binary not found: $exe"
}

New-Item -ItemType Directory -Force -Path $output | Out-Null

function Wait-ForServer {
    param([System.Diagnostics.Process]$Process)

    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        Start-Sleep -Milliseconds 100
        $ErrorActionPreference = 'Continue'
        & $exe ping 2>$null | Out-Null
        $pingExit = $LASTEXITCODE
        $ErrorActionPreference = 'Stop'
        if ($pingExit -eq 0) {
            return
        }
        if ($Process.HasExited) {
            throw "Server exited before becoming ready with code $($Process.ExitCode)"
        }
    }
    throw 'Server did not become ready within 10 seconds'
}

function Assert-Stopped {
    param([System.Diagnostics.Process]$Process, [string]$Scenario)

    if (-not $Process.WaitForExit(5000)) {
        throw "$Scenario server did not exit within 5 seconds"
    }
    if (Get-Process -Id $Process.Id -ErrorAction SilentlyContinue) {
        throw "$Scenario process $($Process.Id) is still present"
    }
    if (Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort 43176 -State Listen -ErrorAction SilentlyContinue) {
        throw "$Scenario listener 127.0.0.1:43176 is still present"
    }
}

function Test-Quit {
    param([string]$Scenario, [switch]$Visible)

    $stdout = Join-Path $output "$Scenario-stdout.log"
    $stderr = Join-Path $output "$Scenario-stderr.log"
    $env:SLINT_SHELL_HOTKEYS = 'off'
    $env:SLINT_SHELL_POPUPS = 'winit'
    $process = Start-Process -FilePath $exe -WorkingDirectory $root `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
    Wait-ForServer $process
    if ($Visible) {
        & $exe show emoji | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "$Scenario show failed with exit code $LASTEXITCODE"
        }
        Start-Sleep -Milliseconds 300
    }
    & $exe quit | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "$Scenario quit failed with exit code $LASTEXITCODE"
    }
    Assert-Stopped $process $Scenario
}

$ErrorActionPreference = 'Continue'
& $exe quit 2>$null | Out-Null
$ErrorActionPreference = 'Stop'
Start-Sleep -Milliseconds 300
Test-Quit 'hidden'
Test-Quit 'visible' -Visible

$spellStdout = Join-Path $output 'spell-stdout.log'
$spellStderr = Join-Path $output 'spell-stderr.log'
$env:SLINT_SHELL_POPUPS = 'spell'
$spell = Start-Process -FilePath $exe -WorkingDirectory $root `
    -RedirectStandardOutput $spellStdout -RedirectStandardError $spellStderr -PassThru -Wait
if ($spell.ExitCode -eq 0) {
    throw 'SLINT_SHELL_POPUPS=spell unexpectedly succeeded'
}
$spellMessage = Get-Content $spellStderr -Raw
if ($spellMessage -notmatch 'Spell popups are available only on Linux Wayland') {
    throw "Unexpected spell diagnostic: $spellMessage"
}
if (Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort 43176 -State Listen -ErrorAction SilentlyContinue) {
    throw 'spell failure left listener 127.0.0.1:43176 present'
}

Write-Host 'WINDOWS LIFECYCLE CHECK PASSED' -ForegroundColor Green
Write-Host 'Hidden quit: process and listener released'
Write-Host 'Visible quit: process and listener released'
Write-Host 'Unsupported spell backend: diagnostic and non-zero exit confirmed'
Write-Host "Logs: $output"
