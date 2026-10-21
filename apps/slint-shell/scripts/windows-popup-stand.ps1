# Interactive, isolated shell used by the host's physical-key popup scenarios.
param(
    [Parameter(Mandatory = $true)][string]$Output,
    [ValidateSet('debug', 'release')][string]$BuildProfile = 'debug'
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
. (Join-Path $PSScriptRoot 'windows-test-lib.ps1')
$saved = Initialize-WindowsTest $Output
$process = $null
try {
    $env:LHC_HOTKEYS = 'on'
    $env:SLINT_BACKEND = 'winit-skia'
    $env:LHC_METRICS = Join-Path $Output 'metrics.csv'
    $profileRoot = Join-Path $env:LHC_DEV_DIR 'windows\AppData\Roaming\dev.bozonx.left-hand-control'
    New-Item -ItemType Directory -Force -Path (Join-Path $profileRoot 'layouts') | Out-Null
    $encoding = [Text.UTF8Encoding]::new($false)
    [IO.File]::WriteAllText((Join-Path $profileRoot 'config.json'), '{"settings":{"currentLayoutId":"user:Popup acceptance","manualActiveLayoutId":"user:Popup acceptance","inputDevicePath":"windows:keyboard","startMapperOnLaunch":false,"gameMode":{"control":"off"}}}', $encoding)
    $layout = '{"name":"Popup acceptance","rules":[{"id":"guard-probe","key":"KeyF","tap":"ControlLeft","hold":"ControlLeft"}],"emojiPages":[{"id":"emoji","name":"Unicode","cells":{"KeyQ":"\u2615","KeyW":"\ud83d\ude00","KeyE":"\ud83d\udc69\u200d\ud83d\udcbb"}}],"quickActionPages":[{"id":"quick","name":"Text","cells":{"KeyQ":{"name":"Mixed text","action":"text:\u0414\u0435\u0439\u0441\u0442\u0432\u0438\u0435 02 \ud83d\ude00"}}}]}'
    [IO.File]::WriteAllText((Join-Path $profileRoot 'layouts\Popup acceptance.json'), $layout, $encoding)
    $exe = Join-Path $root "target\$BuildProfile\left-hand-control.exe"
    $process = Start-Process $exe -WorkingDirectory $root -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $Output 'stdout.log') `
        -RedirectStandardError (Join-Path $Output 'stderr.log')
    $null = $process.Handle
    $deadline = (Get-Date).AddSeconds(30)
    do {
        $ErrorActionPreference = 'Continue'
        & $exe ping 2>$null | Out-Null
        $ready = $LASTEXITCODE -eq 0
        $ErrorActionPreference = 'Stop'
        if ($process.HasExited) { throw 'Popup shell exited during startup' }
        if ((Get-Date) -gt $deadline) { throw 'Popup shell did not become ready' }
        if (-not $ready) { Start-Sleep -Milliseconds 100 }
    } while (-not $ready)
    Assert-WindowsTestOwner $process
    @{ pid = $process.Id; port = $env:LHC_PORT; profile = $env:LHC_DEV_DIR; exe = $exe } |
        ConvertTo-Json -Compress | Set-Content (Join-Path $Output 'ready.json') -Encoding utf8
    if (-not $process.WaitForExit(1200000)) { throw 'Popup host checks timed out' }
    if ($process.ExitCode -ne 0 -and -not (Test-Path (Join-Path $Output 'expect-crash'))) {
        throw "Popup shell exited with $($process.ExitCode)"
    }
} finally {
    try { Stop-WindowsTestProcess $process } finally { Restore-WindowsTest $saved }
}
