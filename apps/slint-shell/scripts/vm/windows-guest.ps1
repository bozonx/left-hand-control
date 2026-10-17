# Guest side of the Windows VM e2e stand. The scheduled task LHC-E2E runs it in
# the interactive session of the autologon user; run-windows.py waits for
# D:\lhc-e2e\out\done.txt and reads summary.txt.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$out = 'D:\lhc-e2e\out'
New-Item -ItemType Directory -Force -Path $out | Out-Null
Remove-Item "$out\*" -Recurse -Force -ErrorAction SilentlyContinue
$summary = Join-Path $out 'summary.txt'
$log = Join-Path $out 'automated.log'
$code = 1
$receiver = $null

function Add-Line([string]$Text) {
    Add-Content -Path $summary -Value $Text -Encoding utf8
}

try {
    # Scheduled tasks otherwise default to BelowNormal and slow native builds.
    [System.Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Normal'
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
    $env:CARGO_HOME = 'D:\lhc-e2e\cargo'
    # Keep native Slint builds within the VM memory budget.
    $env:CARGO_BUILD_JOBS = '2'
    New-Item -ItemType Directory -Force -Path $env:CARGO_HOME | Out-Null
    $os = Get-CimInstance Win32_OperatingSystem
    Add-Line "Windows: $($os.Caption) build $($os.BuildNumber)"
    Add-Line "Session: $([System.Diagnostics.Process]::GetCurrentProcess().SessionId)"
    if ([System.Diagnostics.Process]::GetCurrentProcess().SessionId -eq 0) { throw 'Interactive session required' }
    Add-Line "Cargo: $(& cargo --version 2>&1)"
    Add-Line "Rust: $(& rustc --version 2>&1)"
    Add-Line ''

    $suite = (Get-Content 'D:\lhc-e2e\suite.txt' -Raw).Trim()
    if ($suite -notin @('smoke', 'full')) { throw "Unknown suite: $suite" }
    $script = if ($suite -eq 'smoke') { 'check-windows-smoke.ps1' } else { 'check-windows-automated.ps1' }
    Add-Line "Suite: $suite"

    # Native commands write progress to stderr; the exit code is the verdict.
    $ErrorActionPreference = 'Continue'
    & powershell -NoProfile -ExecutionPolicy Bypass -File `
        (Join-Path $root "apps\slint-shell\scripts\$script") *>&1 |
        Out-File -FilePath $log -Encoding utf8
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($code -eq 0) {
        Add-Line "- ${suite}: passed"
    } else {
        Add-Line "- ${suite}: FAILED (exit $code, see automated.log)"
    }

    if ($code -eq 0 -and $suite -eq 'smoke') {
        $receiverScript = Join-Path $root 'apps\slint-shell\scripts\windows-receiver.ps1'
        $receiver = Start-Process powershell -PassThru -ArgumentList @(
            '-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-File', $receiverScript,
            '-Output', "$out\receiver", '-InputLocale', 'en-US'
        ) -RedirectStandardOutput "$out\receiver-stdout.log" -RedirectStandardError "$out\receiver-stderr.log"
        $deadline = (Get-Date).AddMinutes(3)
        while (-not (Test-Path "$out\input-result.txt")) {
            if ($receiver.HasExited) { throw 'Input receiver exited before host checks' }
            if ((Get-Date) -gt $deadline) { throw 'Host keyboard checks timed out' }
            Start-Sleep -Milliseconds 200
        }
        if ((Get-Content "$out\input-result.txt" -Raw).Trim() -ne '0') {
            throw 'Host keyboard checks failed; see host report and receiver events'
        }
        Add-Line '- hardware-keyboard: passed (native input, Shift, Ctrl+Home, Shift+End, key release)'
    }

    $lifecycle = Join-Path $root 'windows-runtime'
    if (Test-Path $lifecycle) {
        Copy-Item $lifecycle (Join-Path $out 'runtime') -Recurse -Force
    }
} catch {
    $code = 1
    Add-Line "- harness: FAILED ($_)"
} finally {
    if ($receiver -and -not $receiver.HasExited) {
        $receiver.CloseMainWindow() | Out-Null
        if (-not $receiver.WaitForExit(5000)) { Stop-Process -Id $receiver.Id -Force }
    }
    Set-Content -Path (Join-Path $out 'done.txt') -Value $code
}
