# Shared isolation and process ownership checks for native Windows acceptance.
function Initialize-WindowsTest([string]$Output) {
    if ([System.Diagnostics.Process]::GetCurrentProcess().SessionId -eq 0) {
        throw 'Run acceptance in the interactive desktop session.'
    }
    $principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Run acceptance without elevation.'
    }
    $saved = @{}
    foreach ($name in @('TECLOID_DEV_DIR', 'TECLOID_PORT', 'TECLOID_HOTKEYS', 'TECLOID_POPUPS', 'TECLOID_METRICS', 'SLINT_BACKEND')) {
        $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
    }
    New-Item -ItemType Directory -Force -Path $Output | Out-Null
    $env:TECLOID_DEV_DIR = Join-Path $Output ([guid]::NewGuid().ToString())
    $env:TECLOID_HOTKEYS = 'off'
    $env:TECLOID_POPUPS = 'winit'
    $env:SLINT_BACKEND = 'winit-skia-software'
    Remove-Item Env:TECLOID_METRICS -ErrorAction SilentlyContinue
    $listener = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    try { $env:TECLOID_PORT = [string]$listener.LocalEndpoint.Port } finally { $listener.Stop() }
    return $saved
}

# Popup scenarios explicitly opt in; ordinary keymaps keep the product default.
function Enable-WindowsTestDevelopment {
    $keymapRoot = Join-Path $env:TECLOID_DEV_DIR 'windows\AppData\Roaming\dev.bozonx.tecloid'
    New-Item -ItemType Directory -Force -Path $keymapRoot | Out-Null
    [IO.File]::WriteAllText((Join-Path $keymapRoot 'config.json'), '{"version":1,"settings":{"experimental":true,"startMapperOnLaunch":false},"state":{"gameModeSwitch":"off"}}', [Text.UTF8Encoding]::new($false))
}

function Restore-WindowsTest($Saved) {
    foreach ($name in $Saved.Keys) {
        [Environment]::SetEnvironmentVariable($name, $Saved[$name], 'Process')
    }
}

function Assert-WindowsTestOwner([System.Diagnostics.Process]$Process) {
    if ($Process.HasExited) { throw "Test process exited with code $($Process.ExitCode)" }
    $listeners = @(Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort ([int]$env:TECLOID_PORT) -State Listen -ErrorAction SilentlyContinue)
    if ($listeners.Count -ne 1 -or $listeners[0].OwningProcess -ne $Process.Id) {
        throw 'IPC listener does not belong to the test process'
    }
}

function Stop-WindowsTestProcess([System.Diagnostics.Process]$Process) {
    if ($Process -and -not $Process.HasExited) {
        Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
        if (-not $Process.WaitForExit(10000)) { throw "Could not stop test process $($Process.Id)" }
    }
}
