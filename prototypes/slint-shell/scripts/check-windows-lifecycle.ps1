$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$exe = Join-Path $root 'prototypes\slint-shell\target\release\slint-shell.exe'
$output = Join-Path $root 'windows-runtime\lifecycle'

if (-not (Test-Path $exe)) {
    throw "Release binary not found: $exe"
}

New-Item -ItemType Directory -Force -Path $output | Out-Null
$env:SLINT_BACKEND = 'winit-skia'

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

function Invoke-Client {
    param([string]$Command)

    $arguments = $Command.Split(' ')
    & $exe @arguments | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "IPC command '$Command' failed with exit code $LASTEXITCODE"
    }
}

function Test-Stress {
    $scenario = 'stress'
    $stdout = Join-Path $output "$scenario-stdout.log"
    $stderr = Join-Path $output "$scenario-stderr.log"
    $metrics = Join-Path $output "$scenario-metrics.csv"
    Remove-Item $metrics -ErrorAction SilentlyContinue
    $env:SLINT_SHELL_HOTKEYS = 'off'
    $env:SLINT_SHELL_POPUPS = 'winit'
    $env:SLINT_SHELL_METRICS = $metrics
    $process = Start-Process -FilePath $exe -WorkingDirectory $root `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
    Wait-ForServer $process

    $samples = @()
    for ($cycle = 1; $cycle -le 100; $cycle++) {
        Invoke-Client $(if ($cycle % 2) { 'show emoji' } else { 'show quick' })
        Invoke-Client 'hide'
        if ($cycle % 10 -eq 0) {
            Start-Sleep -Milliseconds 100
            $process.Refresh()
            $samples += [pscustomobject]@{
                Cycle = $cycle
                WorkingSet = $process.WorkingSet64
                Handles = $process.HandleCount
            }
        }
    }

    for ($attempt = 0; $attempt -lt 50; $attempt++) {
        $completed = @(Import-Csv $metrics | Where-Object event -eq 'hidden').Count
        if ($completed -eq 100) { break }
        Start-Sleep -Milliseconds 100
    }
    if ($completed -ne 100) {
        throw "Only $completed of 100 popup cycles completed"
    }

    $shell = New-Object -ComObject WScript.Shell
    $shell.SendKeys('^%{F11}')
    $shell.SendKeys('^%{F12}')
    Start-Sleep -Milliseconds 500

    $rows = Import-Csv $metrics
    if (@($rows | Where-Object source -eq 'hotkey').Count -ne 0) {
        throw 'SLINT_SHELL_HOTKEYS=off still produced a hotkey popup trigger'
    }
    $triggers = @($rows | Where-Object event -eq 't0_trigger')
    $hidden = @($rows | Where-Object event -eq 'hidden')
    if ($triggers.Count -ne 100 -or $hidden.Count -ne 100) {
        throw "Expected 100 trigger/hidden rows, got $($triggers.Count)/$($hidden.Count)"
    }
    if (@($triggers | Where-Object window -eq 'emoji').Count -ne 50 -or
        @($triggers | Where-Object window -eq 'quick').Count -ne 50) {
        throw 'Expected 50 Emoji and 50 Quick trials'
    }

    $warm = $samples | Select-Object -Skip 1
    $workingSetGrowth = ($warm | Select-Object -Last 1).WorkingSet - ($warm | Select-Object -First 1).WorkingSet
    $handleGrowth = ($warm | Select-Object -Last 1).Handles - ($warm | Select-Object -First 1).Handles
    if ($workingSetGrowth -gt 64MB) {
        throw "Working set grew by $workingSetGrowth bytes after warm-up"
    }
    if ($handleGrowth -gt 32) {
        throw "Handle count grew by $handleGrowth after warm-up"
    }
    $samples | Export-Csv (Join-Path $output 'stress-resources.csv') -NoTypeInformation

    Invoke-Client 'quit'
    Assert-Stopped $process $scenario
    Remove-Item Env:SLINT_SHELL_METRICS -ErrorAction SilentlyContinue
}

$ErrorActionPreference = 'Continue'
& $exe quit 2>$null | Out-Null
$ErrorActionPreference = 'Stop'
Start-Sleep -Milliseconds 300
Test-Quit 'hidden'
Test-Quit 'visible' -Visible
Test-Stress

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
Write-Host '100 show/hide cycles: metrics, working set and handles checked'
Write-Host 'Disabled hotkeys: Ctrl+Alt+F11/F12 produced no popup trigger'
Write-Host 'Unsupported spell backend: diagnostic and non-zero exit confirmed'
Write-Host "Logs: $output"
