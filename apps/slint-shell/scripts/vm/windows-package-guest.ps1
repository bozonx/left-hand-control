# ISO bootstrap: run in the clean VM's ordinary interactive desktop.
$ErrorActionPreference = 'Stop'
$settings = Get-Content (Join-Path $PSScriptRoot 'package-test.json') -Raw | ConvertFrom-Json
$root = Join-Path $env:USERPROFILE 'lhc-package-check'
New-Item -ItemType Directory -Force $root | Out-Null
$transcript = Join-Path $root 'bootstrap.log'
Start-Transcript -Path $transcript | Out-Null
$code = '1'
function Send-Result([string]$Phase, [string]$Status, [string]$Details) {
    $body = [Text.Encoding]::UTF8.GetBytes((@{phase=$Phase; status=$Status; details=$Details} | ConvertTo-Json -Compress))
    Invoke-RestMethod -Method Post -Uri $settings.callback -ContentType 'application/json' -Body $body -TimeoutSec 15 | Out-Null
}
try {
    $tools = @(Get-Command cargo,rustc,cl,devenv -ErrorAction SilentlyContinue)
    $vs = @('C:\Program Files\Microsoft Visual Studio', 'C:\Program Files (x86)\Microsoft Visual Studio', (Join-Path $env:USERPROFILE '.rustup')) | Where-Object { Test-Path $_ }
    if ($tools.Count -or $vs.Count) { throw 'Clean baseline contains developer tools' }
    Write-Host 'Clean baseline: no Rust/Cargo/MSVC/Visual Studio toolchain'
    Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber | Format-List | Out-Host
    Copy-Item (Join-Path $PSScriptRoot 'apps') $root -Recurse -Force
    Copy-Item (Join-Path $PSScriptRoot '*.zip') $root -Force
    Copy-Item (Join-Path $PSScriptRoot 'WINDOWS-SHA256SUMS') $root -Force
    $runtimeBefore = Test-Path "$env:windir\System32\MSVCP140.dll"
    Write-Host "VC runtime present before test: $runtimeBefore"
    if (-not $runtimeBefore) {
        $installer = Join-Path $PSScriptRoot 'vc_redist.x64.exe'
        $signature = Get-AuthenticodeSignature $installer
        if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') {
            throw 'VC runtime installer does not have a valid Microsoft signature'
        }
        Send-Result 'runtime-uac' '' 'Install the signed VC runtime in this disposable VM'
        $install = Start-Process $installer -Verb RunAs -PassThru -ArgumentList '/install /quiet /norestart'
        if (-not $install.WaitForExit(180000)) { throw 'VC runtime install timed out' }
        if ($install.ExitCode -notin @(0,3010)) { throw "VC runtime installer exit $($install.ExitCode)" }
        if (-not (Test-Path "$env:windir\System32\MSVCP140.dll")) { throw 'VC runtime DLL still absent' }
    }
    $archive = Get-Item (Join-Path $root '*.zip')
    & (Join-Path $root 'apps\slint-shell\scripts\windows\check-windows-syntax.ps1')
    & (Join-Path $root 'apps\slint-shell\scripts\windows\check-windows-package.ps1') -Archive $archive.FullName -Desktop -StressCycles $settings.cycles -StressSeconds $settings.seconds
    $configs = @(Get-ChildItem (Join-Path $root 'windows-runtime') -Recurse -File -Filter config.json)
    if (-not $configs.Count) { throw 'Portable removal lost the isolated test profiles' }
    Write-Host 'Package extraction removed, isolated configuration retained'
    $code = '0'
} catch { Write-Host "FAILED: $_" } finally {
    Stop-Transcript | Out-Null
    $details = Get-Content $transcript -Raw
    Send-Result 'complete' $code $details
}
