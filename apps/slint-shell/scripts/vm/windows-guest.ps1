# Guest side of the Windows VM e2e stand. The scheduled task LHC-E2E runs it in
# the interactive session of the autologon user; run-windows.py waits for
# D:\lhc-e2e\out\done.txt and reads summary.txt.
$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$out = 'D:\lhc-e2e\out'
New-Item -ItemType Directory -Force -Path $out | Out-Null
Remove-Item "$out\*" -Recurse -Force -ErrorAction SilentlyContinue
$summary = Join-Path $out 'summary.txt'
$log = Join-Path $out 'automated.log'

function Add-Line([string]$Text) {
    Add-Content -Path $summary -Value $Text -Encoding utf8
}

try {
    $os = Get-CimInstance Win32_OperatingSystem
    Add-Line "Windows: $($os.Caption) build $($os.BuildNumber)"
    Add-Line "Rust: $(& rustc --version 2>&1)"
    Add-Line ''

    # Native commands write progress to stderr; the exit code is the verdict.
    $ErrorActionPreference = 'Continue'
    & powershell -NoProfile -ExecutionPolicy Bypass -File `
        (Join-Path $root 'apps\slint-shell\scripts\check-windows-automated.ps1') *>&1 |
        Out-File -FilePath $log -Encoding utf8
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($code -eq 0) {
        Add-Line '- check-windows-automated: passed (tests, release build, interactions, lifecycle)'
    } else {
        Add-Line "- check-windows-automated: FAILED (exit $code, see automated.log)"
    }

    $lifecycle = Join-Path $root 'windows-runtime\lifecycle'
    if (Test-Path $lifecycle) {
        Copy-Item $lifecycle (Join-Path $out 'lifecycle') -Recurse -Force
    }
} catch {
    Add-Line "- harness: FAILED ($_)"
} finally {
    Set-Content -Path (Join-Path $out 'done.txt') -Value (Get-Date -Format o)
}
