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
$mapper = $null
$popupStand = $null

function Add-Line([string]$Text) {
    Add-Content -Path $summary -Value $Text -Encoding utf8
}

try {
    # Scheduled tasks otherwise default to BelowNormal and slow native builds.
    [System.Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Normal'
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
    $env:CARGO_HOME = 'D:\lhc-e2e\cargo'
    # Keep native Slint builds within the VM memory budget.
    $env:CARGO_BUILD_JOBS = '1'
    New-Item -ItemType Directory -Force -Path $env:CARGO_HOME | Out-Null
    $os = Get-CimInstance Win32_OperatingSystem
    Add-Line "Windows: $($os.Caption) build $($os.BuildNumber)"
    Add-Line "Session: $([System.Diagnostics.Process]::GetCurrentProcess().SessionId)"
    if ([System.Diagnostics.Process]::GetCurrentProcess().SessionId -eq 0) { throw 'Interactive session required' }
    Add-Line "Cargo: $(& cargo --version 2>&1)"
    Add-Line "Rust: $(& rustc --version 2>&1)"
    Add-Line ''

    $suite = (Get-Content 'D:\lhc-e2e\suite.txt' -Raw).Trim()
    if ($suite -notin @('smoke', 'full', 'release', 'mapper')) { throw "Unknown suite: $suite" }
    $script = 'windows\check-windows-smoke.ps1'
    Add-Line "Suite: $suite"

    # Native commands write progress to stderr; the exit code is the verdict.
    $ErrorActionPreference = 'Continue'
    if ($suite -eq 'mapper') {
        $step = 'core-driver-build'
        & cargo build --locked --manifest-path (Join-Path $root 'crates\lhc-core\Cargo.toml') `
            --example windows-mapper-stand *>&1 | Out-File -FilePath $log -Encoding utf8
    } elseif ($suite -ne 'release') {
        $step = 'shell-smoke'
        & powershell -NoProfile -ExecutionPolicy Bypass -File `
            (Join-Path $root "apps\slint-shell\scripts\$script") *>&1 |
            Out-File -FilePath $log -Encoding utf8
    }
    $code = if ($suite -eq 'release') { 0 } else { $LASTEXITCODE }
    $ErrorActionPreference = 'Stop'
    if ($suite -eq 'release') {
        Add-Line '- debug smoke/native tests/clippy/package lifecycle: not included in release input suite'
    } elseif ($code -eq 0) {
        Add-Line "- ${step}: passed"
    } else {
        Add-Line "- ${step}: FAILED (exit $code, see automated.log)"
    }

    if ($code -eq 0) {
        if ($suite -in @('full', 'release')) {
            $ErrorActionPreference = 'Continue'
            & cargo build --release --locked --manifest-path (Join-Path $root 'Cargo.toml') `
                -p lhc-core -p slint-shell --bin left-hand-control --example windows-mapper-stand *>&1 |
                Out-File -FilePath (Join-Path $out 'mapper-build.log') -Encoding utf8
            $driverCode = $LASTEXITCODE
            $ErrorActionPreference = 'Stop'
            if ($driverCode -ne 0) { throw "Release mapper driver build failed (exit $driverCode)" }
            Add-Line '- release-mapper-driver-build: passed'
            Add-Line '- release-shell-build: passed'
        }
        $driverProfile = if ($suite -in @('full', 'release')) { 'release' } else { 'debug' }
        $receiverScript = Join-Path $root 'apps\slint-shell\scripts\windows\windows-receiver.ps1'
        $receiver = Start-Process powershell -PassThru -ArgumentList @(
            '-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-File', $receiverScript,
            '-Output', "$out\receiver", '-InputLocale', 'en-US'
        ) -RedirectStandardOutput "$out\receiver-stdout.log" -RedirectStandardError "$out\receiver-stderr.log"
        if ($suite -ne 'mapper') {
            $popupScript = Join-Path $root 'apps\slint-shell\scripts\windows\windows-popup-stand.ps1'
            $popupStand = Start-Process powershell -PassThru -ArgumentList @(
                '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $popupScript, '-Output', "$out\popups",
                '-BuildProfile', $driverProfile
            ) -RedirectStandardOutput "$out\popups-stdout.log" -RedirectStandardError "$out\popups-stderr.log"
            $null = $popupStand.Handle
        }
        $stand = Join-Path $root "target\$driverProfile\examples\windows-mapper-stand.exe"
        $previousDevDir = [Environment]::GetEnvironmentVariable('LHC_DEV_DIR', 'Process')
        try {
            $env:LHC_DEV_DIR = Join-Path $out 'mapper-profile'
            $mapper = Start-Process $stand -PassThru -WindowStyle Hidden -ArgumentList "$out\mapper" `
                -RedirectStandardOutput "$out\mapper-stdout.log" -RedirectStandardError "$out\mapper-stderr.log"
        } finally { [Environment]::SetEnvironmentVariable('LHC_DEV_DIR', $previousDevDir, 'Process') }
        $null = $mapper.Handle
        $stressSeconds = if (Test-Path 'D:\lhc-e2e\stress-seconds.txt') { [int](Get-Content 'D:\lhc-e2e\stress-seconds.txt' -Raw) } else { 180 }
        if ($stressSeconds -lt 1 -or $stressSeconds -gt 604800) { throw 'Invalid stress duration' }
        $deadline = (Get-Date).AddSeconds(3600 + $stressSeconds)
        while (-not (Test-Path "$out\input-result.txt")) {
            if ($popupStand -and $popupStand.HasExited -and $popupStand.ExitCode -ne 0) {
                throw "Popup acceptance stand failed (exit $($popupStand.ExitCode), see popups-stderr.log)"
            }
            if ($mapper.HasExited) { throw 'Mapper stand exited before host checks' }
            if ($receiver.HasExited) { throw 'Input receiver exited before host checks' }
            if ((Get-Date) -gt $deadline) { throw 'Host keyboard checks timed out' }
            Start-Sleep -Milliseconds 200
        }
        if ((Get-Content "$out\input-result.txt" -Raw).Trim() -ne '0') {
            throw 'Host keyboard checks failed; see host report and receiver events'
        }
        Add-Line '- hardware-keyboard and mapper scenarios: passed (see named host steps)'
        if ($popupStand -and (-not $popupStand.WaitForExit(10000) -or $popupStand.ExitCode -ne 0)) {
            throw 'Popup acceptance stand did not exit cleanly'
        }
        [IO.File]::WriteAllText("$out\mapper\request.json", '{"id":"guest-shutdown","op":"shutdown"}', [Text.UTF8Encoding]::new($false))
        if (-not $mapper.WaitForExit(10000) -or $mapper.ExitCode -ne 0) { throw 'Mapper stand did not shut down cleanly' }
        $receiver.CloseMainWindow() | Out-Null
        if (-not $receiver.WaitForExit(5000)) { Stop-Process -Id $receiver.Id -Force }

    }

    if ($code -eq 0 -and $suite -eq 'full') {
        # Functional input checks run first; the heavier compile matrix follows.
        $ErrorActionPreference = 'Continue'
        & powershell -NoProfile -ExecutionPolicy Bypass -File `
            (Join-Path $root 'apps\slint-shell\scripts\windows\check-windows-automated.ps1') -SkipSmoke *>&1 |
            Out-File -FilePath (Join-Path $out 'full.log') -Encoding utf8
        $code = $LASTEXITCODE
        $ErrorActionPreference = 'Stop'
        if ($code -ne 0) { throw "Full native/release checks failed (exit $code, see full.log)" }
        Add-Line '- full-native-release: passed'
        $packages = Join-Path $root 'target\release-artifacts-windows'
        if (Test-Path $packages) { Copy-Item $packages (Join-Path $out 'packages') -Recurse -Force }
    }

    $lifecycle = Join-Path $root 'windows-runtime'
    if (Test-Path $lifecycle) {
        Copy-Item $lifecycle (Join-Path $out 'runtime') -Recurse -Force
    }
} catch {
    $code = 1
    Add-Line "- harness: FAILED ($_)"
} finally {
    if ($mapper -and -not $mapper.HasExited) { Stop-Process -Id $mapper.Id -Force }
    if ($receiver -and -not $receiver.HasExited) {
        $receiver.CloseMainWindow() | Out-Null
        if (-not $receiver.WaitForExit(5000)) { Stop-Process -Id $receiver.Id -Force }
    }
    Set-Content -Path (Join-Path $out 'done.txt') -Value $code
    $callback = 'D:\lhc-e2e\completion-url.txt'
    if (Test-Path $callback) {
        try {
            $fullLog = if (Test-Path "$out\full.log") { Get-Content "$out\full.log" -Raw -Encoding utf8 } else { '' }
            $result = @{ status = [string]$code; summary = [string](Get-Content $summary -Raw -Encoding utf8); full_log = [string]$fullLog }
            $body = [Text.Encoding]::UTF8.GetBytes(($result | ConvertTo-Json -Compress))
            Invoke-RestMethod -Method Post -Uri ((Get-Content $callback -Raw).Trim()) `
                -ContentType 'application/json' -Body $body -TimeoutSec 15 | Out-Null
        } catch {
            # done.txt remains authoritative for runners using SSH.
            Add-Content "$out\completion-error.log" -Value "$_" -Encoding utf8
        }
    }
}
