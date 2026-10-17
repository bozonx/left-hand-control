# Prepare the Windows 11 test VM once, before its base image is frozen: OpenSSH
# key login with PowerShell as the shell, autologon of the test user, no sleep,
# the cache disk as D: and the interactive scheduled task LHC-E2E that
# run-windows.py starts. Run in an elevated PowerShell inside the guest:
#   .\prepare-windows-guest.ps1 -User user -Password '<password>' -PublicKey '<ssh public key>'
param(
    [Parameter(Mandatory = $true)][string]$User,
    [Parameter(Mandatory = $true)][string]$Password,
    [Parameter(Mandatory = $true)][string]$PublicKey
)
$ErrorActionPreference = 'Stop'

Write-Host 'OpenSSH server...'
Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0 | Out-Null
Set-Service sshd -StartupType Automatic
Start-Service sshd
New-ItemProperty -Path 'HKLM:\SOFTWARE\OpenSSH' -Name DefaultShell -Force `
    -Value 'C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe' | Out-Null

$isAdmin = (Get-LocalGroupMember -Group Administrators |
    Where-Object { $_.Name -like "*\$User" }) -ne $null
if ($isAdmin) {
    $keys = 'C:\ProgramData\ssh\administrators_authorized_keys'
    Set-Content -Path $keys -Value $PublicKey -Encoding ascii
    icacls $keys /inheritance:r /grant 'Administrators:F' /grant 'SYSTEM:F' | Out-Null
} else {
    $dir = "C:\Users\$User\.ssh"
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    Set-Content -Path "$dir\authorized_keys" -Value $PublicKey -Encoding ascii
    icacls "$dir\authorized_keys" /inheritance:r /grant "${User}:F" /grant 'SYSTEM:F' | Out-Null
}

Write-Host 'Autologon...'
$winlogon = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
Set-ItemProperty $winlogon AutoAdminLogon '1'
Set-ItemProperty $winlogon DefaultUserName $User
Set-ItemProperty $winlogon DefaultPassword $Password
Set-ItemProperty $winlogon DefaultDomainName $env:COMPUTERNAME

Write-Host 'Power settings...'
powercfg /change standby-timeout-ac 0
powercfg /change monitor-timeout-ac 0
powercfg /change hibernate-timeout-ac 0

Write-Host 'Cache disk...'
$raw = Get-Disk | Where-Object PartitionStyle -eq 'RAW' | Select-Object -First 1
if ($raw) {
    $raw | Initialize-Disk -PartitionStyle GPT -PassThru |
        New-Partition -DriveLetter D -UseMaximumSize |
        Format-Volume -FileSystem NTFS -NewFileSystemLabel 'LHC-CACHE' -Confirm:$false | Out-Null
}
if (-not (Test-Path 'D:\')) {
    throw 'D: is missing: attach cache.qcow2 (cache_disk in vm.json) and run again.'
}
New-Item -ItemType Directory -Force -Path 'D:\lhc-e2e\src', 'D:\lhc-e2e\out' | Out-Null
icacls 'D:\lhc-e2e' /grant "${User}:(OI)(CI)F" | Out-Null

Write-Host 'Scheduled task LHC-E2E...'
$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument `
    '-NoProfile -ExecutionPolicy Bypass -File D:\lhc-e2e\src\apps\slint-shell\scripts\vm\windows-guest.ps1'
$principal = New-ScheduledTaskPrincipal -UserId $User -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Hours 3) `
    -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
Register-ScheduledTask -TaskName 'LHC-E2E' -Action $action -Principal $principal `
    -Settings $settings -Force | Out-Null

$cargo = "C:\Users\$User\.cargo\bin\cargo.exe"
if (-not (Test-Path $cargo)) {
    Write-Warning "Rust is not installed for $User; see docs/testing/windows.md, Toolchain installation."
}
Write-Host 'Prepared. Reboot once to confirm autologon, then shut down and freeze the disk.'
