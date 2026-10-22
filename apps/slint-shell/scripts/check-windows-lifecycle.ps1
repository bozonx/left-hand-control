param(
    [string]$Executable,
    [ValidateRange(100, 10000)][int]$StressCycles = 100,
    [ValidateRange(0, 86400)][int]$StressSeconds = 0
)
$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$exe = if ($Executable) { (Resolve-Path $Executable).Path } else { Join-Path $root 'target\release\left-hand-control.exe' }
$workingDirectory = if ($Executable) { Split-Path $exe } else { $root }
$output = Join-Path $root 'windows-runtime\lifecycle'

if (-not (Test-Path $exe)) {
    throw "Release binary not found: $exe"
}

New-Item -ItemType Directory -Force -Path $output | Out-Null
. (Join-Path $PSScriptRoot 'windows-test-lib.ps1')
$saved = Initialize-WindowsTest $output
$env:SLINT_BACKEND = 'winit-skia'
$script:processes = [System.Collections.Generic.List[System.Diagnostics.Process]]::new()

function Wait-ForServer {
    param([System.Diagnostics.Process]$Process)

    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        Start-Sleep -Milliseconds 100
        $ErrorActionPreference = 'Continue'
        & $exe ping 2>$null | Out-Null
        $pingExit = $LASTEXITCODE
        $ErrorActionPreference = 'Stop'
        if ($pingExit -eq 0) {
            Assert-WindowsTestOwner $Process
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
    if ($Process.ExitCode -ne 0) { throw "$Scenario exited with $($Process.ExitCode)" }
    if (Get-Process -Id $Process.Id -ErrorAction SilentlyContinue) {
        throw "$Scenario process $($Process.Id) is still present"
    }
    if (Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort ([int]$env:LHC_PORT) -State Listen -ErrorAction SilentlyContinue) {
        throw "$Scenario test IPC listener is still present"
    }
}

function Test-Quit {
    param([string]$Scenario, [switch]$Visible)

    $stdout = Join-Path $output "$Scenario-stdout.log"
    $stderr = Join-Path $output "$Scenario-stderr.log"
    $env:LHC_HOTKEYS = 'off'
    $env:LHC_POPUPS = 'winit'
    $env:LHC_METRICS = Join-Path $output "$Scenario-metrics.csv"
    Remove-Item $env:LHC_METRICS -ErrorAction SilentlyContinue
    $process = Start-Process -FilePath $exe -WorkingDirectory $workingDirectory `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
    $null = $process.Handle
    $script:processes.Add($process)
    Wait-ForServer $process
    Invoke-Client 'preferences dark ru'
    Invoke-Client 'game on'
    $isolatedConfig = Join-Path $env:LHC_DEV_DIR 'windows\AppData\Roaming\dev.bozonx.left-hand-control\config.json'
    $deadline = (Get-Date).AddSeconds(10)
    while (-not (Test-Path $isolatedConfig)) {
        if ((Get-Date) -gt $deadline) { throw 'Release did not save to isolated storage' }
        Start-Sleep -Milliseconds 100
    }
    Invoke-Client 'game auto'
    if ($Visible) {
        & $exe show emoji | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "$Scenario show failed with exit code $LASTEXITCODE"
        }
        Wait-Metric 't2_shown' 1
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
    $env:LHC_HOTKEYS = 'off'
    $env:LHC_POPUPS = 'winit'
    $env:LHC_METRICS = $metrics
    $process = Start-Process -FilePath $exe -WorkingDirectory $workingDirectory `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
    $null = $process.Handle
    $script:processes.Add($process)
    Wait-ForServer $process

    $samples = @()
    $stressDeadline = (Get-Date).AddSeconds($StressSeconds)
    for ($cycle = 1; $cycle -le $StressCycles -or (Get-Date) -lt $stressDeadline; $cycle++) {
        Invoke-Client $(if ($cycle % 2) { 'show emoji' } else { 'show quick' })
        Wait-Metric 't2_shown' $cycle
        Invoke-Client 'hide'
        Wait-Metric 'hidden' $cycle
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

    $cycleCount = $cycle - 1
    for ($attempt = 0; $attempt -lt 50; $attempt++) {
        $completed = @(Import-Csv $metrics | Where-Object event -eq 'hidden').Count
        if ($completed -eq $cycleCount) { break }
        Start-Sleep -Milliseconds 100
    }
    if ($completed -ne $cycleCount) {
        throw "Only $completed of $cycleCount popup cycles completed"
    }

    $shell = New-Object -ComObject WScript.Shell
    $shell.SendKeys('^%{F11}')
    $shell.SendKeys('^%{F12}')
    Start-Sleep -Milliseconds 500

    $rows = Import-Csv $metrics
    if (@($rows | Where-Object source -eq 'hotkey').Count -ne 0) {
        throw 'LHC_HOTKEYS=off still produced a hotkey popup trigger'
    }
    $triggers = @($rows | Where-Object event -eq 't0_trigger')
    $hidden = @($rows | Where-Object event -eq 'hidden')
    if ($triggers.Count -ne $cycleCount -or $hidden.Count -ne $cycleCount) {
        throw "Expected $cycleCount trigger/hidden rows, got $($triggers.Count)/$($hidden.Count)"
    }
    if (@($triggers | Where-Object window -eq 'emoji').Count -ne [math]::Ceiling($cycleCount / 2) -or
        @($triggers | Where-Object window -eq 'quick').Count -ne [math]::Floor($cycleCount / 2)) {
        throw 'Unexpected Emoji/Quick cycle balance'
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
    Write-Host "Actual popup cycles: $cycleCount; working set growth: $workingSetGrowth; handle growth: $handleGrowth"
    $samples | ConvertTo-Csv -NoTypeInformation | Out-Host

    Invoke-Client 'quit'
    Assert-Stopped $process $scenario
    Remove-Item Env:LHC_METRICS -ErrorAction SilentlyContinue
}

function Wait-Metric([string]$Event, [int]$Count) {
    $deadline = (Get-Date).AddSeconds(10)
    do {
        if (Test-Path $env:LHC_METRICS) {
            $actual = @(Import-Csv $env:LHC_METRICS | Where-Object event -eq $Event).Count
            if ($actual -eq $Count) { return }
            if ($actual -gt $Count) { throw "Duplicate $Event metrics: $actual > $Count" }
        }
        Start-Sleep -Milliseconds 50
    } while ((Get-Date) -lt $deadline)
    throw "Timed out waiting for $Count $Event rows"
}

try {
    Test-Quit 'hidden'
    Test-Quit 'visible' -Visible
    Test-Stress

    $spellStdout = Join-Path $output 'spell-stdout.log'
    $spellStderr = Join-Path $output 'spell-stderr.log'
    $env:LHC_POPUPS = 'spell'
    $spell = Start-Process -FilePath $exe -WorkingDirectory $workingDirectory `
        -RedirectStandardOutput $spellStdout -RedirectStandardError $spellStderr -PassThru
    $null = $spell.Handle
    $script:processes.Add($spell)
    if (-not $spell.WaitForExit(20000)) { throw 'Unsupported Spell startup hung' }
    if ($spell.ExitCode -eq 0) { throw 'LHC_POPUPS=spell unexpectedly succeeded' }
    $spellMessage = Get-Content $spellStderr -Raw
    if ($spellMessage -notmatch 'Spell popups are available only on Linux Wayland') {
        throw "Unexpected spell diagnostic: $spellMessage"
    }
    if (Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort ([int]$env:LHC_PORT) -State Listen -ErrorAction SilentlyContinue) {
        throw 'Spell failure left the test IPC listener present'
    }

    Write-Host 'WINDOWS LIFECYCLE CHECK PASSED' -ForegroundColor Green
    Write-Host "Hidden/visible quit, at least $StressCycles rendered popup cycles over at least ${StressSeconds}s, resource bounds, disabled hotkeys, unsupported Spell"
    Write-Host "Logs: $output"
} finally {
    try {
        foreach ($process in $script:processes) { Stop-WindowsTestProcess $process }
    } finally { Restore-WindowsTest $saved }
}
