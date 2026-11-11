# Interactive, isolated shell used by the host's physical-key popup scenarios.
param(
    [Parameter(Mandatory = $true)][string]$Output,
    [ValidateSet('debug', 'release')][string]$BuildProfile = 'debug'
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
. (Join-Path $PSScriptRoot 'windows-test-lib.ps1')
$saved = Initialize-WindowsTest $Output
$process = $null
try {
    $env:TECLOID_HOTKEYS = 'on'
    $env:SLINT_BACKEND = 'winit-skia'
    $env:TECLOID_METRICS = Join-Path $Output 'metrics.csv'
    $keymapRoot = Join-Path $env:TECLOID_DEV_DIR 'windows\AppData\Roaming\dev.bozonx.tecloid'
    New-Item -ItemType Directory -Force -Path (Join-Path $keymapRoot 'keymaps') | Out-Null
    $encoding = [Text.UTF8Encoding]::new($false)
    [IO.File]::WriteAllText((Join-Path $keymapRoot 'config.json'), '{"version":1,"settings":{"experimental":true,"keymaps":{"manualKeymap":"user:Popup acceptance"},"locale":"en","devices":{"keyboards":["windows:keyboard"]},"startMapperOnLaunch":false},"state":{"editedKeymap":"user:Popup acceptance","gameModeSwitch":"off"}}', $encoding)
    $layout = '{"version":1,"rules":[{"id":"remap-off","key":"KeyA","when":{"gameMode":"off"},"tap":"KeyB"},{"id":"remap-on","key":"KeyA","when":{"gameMode":"on"},"tap":"KeyC"},{"id":"guard-probe","key":"KeyF","tap":"ControlLeft","hold":{"action":"ControlLeft"}}],"emojiMenuPages":[{"id":"emoji","name":"Unicode","cells":{"KeyQ":"\u2615","KeyW":"\ud83d\ude00","KeyE":"\ud83d\udc69\u200d\ud83d\udcbb"}}],"quickMenuPages":[{"id":"quick","name":"Text","cells":{"KeyQ":{"name":"Mixed text","action":"text:\u0414\u0435\u0439\u0441\u0442\u0432\u0438\u0435 02 \ud83d\ude00"}}}]}'
    [IO.File]::WriteAllText((Join-Path $keymapRoot 'keymaps\Popup acceptance.json'), $layout, $encoding)
    $exe = Join-Path $root "target\$BuildProfile\tecloid.exe"
    function Start-Shell {
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
        $metadata = @{ pid = $process.Id; port = $env:TECLOID_PORT; keymap = $env:TECLOID_DEV_DIR; exe = $exe } |
            ConvertTo-Json -Compress
        $readyPath = Join-Path $Output 'ready.json'
        [IO.File]::WriteAllText("$readyPath.tmp", $metadata, $encoding)
        if (Test-Path $readyPath) { [IO.File]::Replace("$readyPath.tmp", $readyPath, [NullString]::Value) }
        else { [IO.File]::Move("$readyPath.tmp", $readyPath) }
        return $process
    }
    $process = Start-Shell
    $deadline = (Get-Date).AddMinutes(20)
    while (-not $process.HasExited) {
        if ((Get-Date) -gt $deadline) { throw 'Popup host checks timed out' }
        $restart = Join-Path $Output 'restart-request'
        if (Test-Path $restart) {
            Remove-Item $restart
            Assert-WindowsTestOwner $process
            & $exe quit
            if ($LASTEXITCODE -ne 0) { throw 'Shell restart quit failed' }
            if (-not $process.WaitForExit(15000)) { throw 'Shell did not quit for restart' }
            if ($process.ExitCode -ne 0) { throw 'Shell restart exit failed' }
            $process = Start-Shell
        }
        Start-Sleep -Milliseconds 100
    }
    if ($process.ExitCode -ne 0 -and -not (Test-Path (Join-Path $Output 'expect-crash'))) {
        throw "Popup shell exited with $($process.ExitCode)"
    }
} finally {
    try { Stop-WindowsTestProcess $process } finally { Restore-WindowsTest $saved }
}
