# Different released versions only. Repacking the same EXE is not an upgrade test.
param(
    [Parameter(Mandatory = $true)][string]$PreviousArchive,
    [Parameter(Mandatory = $true)][string]$Archive
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-test-lib.ps1')
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$output = Join-Path $root 'windows-runtime\upgrade'
$saved = Initialize-WindowsTest $output
$temp = Join-Path $output ([guid]::NewGuid().ToString())
$process = $null
try {
    $executables = @()
    $versions = @()
    foreach ($pair in @(@('previous', $PreviousArchive), @('current', $Archive))) {
        $path = (Resolve-Path $pair[1]).Path
        $checksum = Join-Path (Split-Path $path) 'WINDOWS-SHA256SUMS'
        $expected = "$( (Get-FileHash $path -Algorithm SHA256).Hash.ToLowerInvariant())  $(Split-Path $path -Leaf)"
        if ($expected -notin @(Get-Content $checksum)) { throw 'Upgrade archive checksum mismatch' }
        $destination = Join-Path $temp $pair[0]
        Expand-Archive $path -DestinationPath $destination
        $files = @(Get-ChildItem $destination -Recurse -File -Filter left-hand-control.exe)
        if ($files.Count -ne 1) { throw 'Each ZIP must contain exactly one application executable' }
        $executables += $files[0].FullName
        $version = & $files[0].FullName --version
        if ($LASTEXITCODE -ne 0) { throw 'Version command failed' }
        $versions += [string]$version
    }
    if ($versions[0] -eq $versions[1]) { throw 'Previous and current versions must differ; same-version reinstall does not qualify' }
    foreach ($index in @(0,1)) {
        $exe = $executables[$index]
        $process = Start-Process $exe -WorkingDirectory (Split-Path $exe) -PassThru
        $null = $process.Handle
        $deadline = (Get-Date).AddSeconds(20)
        do {
            $ErrorActionPreference = 'Continue'
            & $exe ping 2>$null | Out-Null
            $ready = $LASTEXITCODE -eq 0
            $ErrorActionPreference = 'Stop'
            if ($process.HasExited -or (Get-Date) -gt $deadline) { throw 'Upgrade application did not become ready' }
            Start-Sleep -Milliseconds 100
        } while (-not $ready)
        Assert-WindowsTestOwner $process
        if ($index -eq 0) {
            & $exe preferences dark ru | Out-Null
            if ($LASTEXITCODE) { throw 'Previous version did not save preferences' }
        }
        & $exe quit | Out-Null
        if ($LASTEXITCODE -or -not $process.WaitForExit(10000) -or $process.ExitCode) { throw 'Upgrade shutdown failed' }
        $profile = Join-Path $env:LHC_DEV_DIR 'windows\AppData\Roaming\dev.bozonx.left-hand-control'
        if ($index -eq 0) {
            $before = @{}
            foreach ($file in Get-ChildItem $profile -Recurse -File) {
                $before[$file.FullName] = (Get-FileHash $file.FullName).Hash
            }
            if (-not $before.Count) { throw 'Previous version saved no isolated configuration' }
        } else {
            foreach ($path in $before.Keys) {
                if (-not (Test-Path $path) -or (Get-FileHash $path).Hash -ne $before[$path]) { throw "Upgrade changed configuration: $path" }
            }
        }
    }
    Remove-Item $temp -Recurse -Force
    foreach ($path in $before.Keys) { if (-not (Test-Path $path)) { throw 'Portable removal lost configuration' } }
    Write-Host "WINDOWS UPGRADE PASSED: $($versions -join ' -> '); configuration retained after portable removal"
} finally {
    try { Stop-WindowsTestProcess $process } finally {
        if (Test-Path $temp) { Remove-Item $temp -Recurse -Force }
        Restore-WindowsTest $saved
    }
}
