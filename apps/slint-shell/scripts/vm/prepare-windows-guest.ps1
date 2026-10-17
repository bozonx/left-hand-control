# Prepare the Windows 11 test VM once, before its base image is frozen: OpenSSH
# key login with PowerShell as the shell, autologon of the test user, no sleep,
# the cache disk as D: and the interactive scheduled task LHC-E2E that
# run-windows.py starts. Run in an elevated PowerShell inside the guest:
#   .\prepare-windows-guest.ps1 -User user -Password '<password>' -PublicKey '<ssh public key>'
param(
    [Parameter(Mandatory = $true)][string]$User,
    [string]$Password,
    [Parameter(Mandatory = $true)][string]$PublicKey,
    [int]$CacheDiskNumber = -1,
    [string]$OpenSshArchive
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

Write-Host 'OpenSSH server...'
if (-not (Get-Service sshd -ErrorAction SilentlyContinue)) {
    if ($OpenSshArchive) {
        $destination = Join-Path $env:ProgramFiles 'OpenSSH'
        if (Test-Path $destination) { throw "Review existing OpenSSH directory first: $destination" }
        $extract = Join-Path $env:TEMP ([guid]::NewGuid().ToString())
        try {
            Expand-Archive -Path $OpenSshArchive -DestinationPath $extract
            $source = Join-Path $extract 'OpenSSH-Win64'
            $signature = Get-AuthenticodeSignature (Join-Path $source 'sshd.exe')
            if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') {
                throw 'OpenSSH server must have a valid Microsoft Authenticode signature.'
            }
            Move-Item $source $destination
            & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $destination 'install-sshd.ps1')
            if ($LASTEXITCODE -ne 0) { throw 'OpenSSH installation failed' }
        } finally {
            Remove-Item $extract -Recurse -Force -ErrorAction SilentlyContinue
        }
    } else {
        Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0 | Out-Null
    }
}
if (-not (Get-NetFirewallRule -Name OpenSSH-Server-In-TCP -ErrorAction SilentlyContinue)) {
    New-NetFirewallRule -Name OpenSSH-Server-In-TCP -DisplayName 'OpenSSH Server (sshd)' `
        -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22 | Out-Null
}
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
if ($PSBoundParameters.ContainsKey('Password')) {
    Set-ItemProperty $winlogon AutoAdminLogon '1'
    Set-ItemProperty $winlogon DefaultUserName $User
    Set-ItemProperty $winlogon DefaultPassword $Password
    Set-ItemProperty $winlogon DefaultDomainName $env:COMPUTERNAME
} else {
    $existing = Get-ItemProperty $winlogon
    if ($existing.AutoAdminLogon -ne '1' -or $existing.DefaultUserName -ne $User) {
        throw 'Pass -Password to configure autologon, or configure it before preparation.'
    }
}

Write-Host 'Power settings...'
powercfg /change standby-timeout-ac 0
powercfg /change monitor-timeout-ac 0
powercfg /change hibernate-timeout-ac 0

Write-Host 'Cache disk...'
if ($CacheDiskNumber -ge 0) {
    $disk = Get-Disk -Number $CacheDiskNumber
    if ($disk.IsBoot -or $disk.IsSystem -or $disk.PartitionStyle -ne 'RAW') {
        throw 'CacheDiskNumber must identify an empty, non-system disk.'
    }
    if (Get-Volume -DriveLetter D -ErrorAction SilentlyContinue) {
        throw 'D: is already in use; detach the installation ISO before preparing the cache.'
    }
    $disk | Initialize-Disk -PartitionStyle GPT -PassThru |
        New-Partition -DriveLetter D -UseMaximumSize |
        Format-Volume -FileSystem NTFS -NewFileSystemLabel 'LHC-CACHE' -Confirm:$false | Out-Null
}
$volume = Get-Volume -DriveLetter D -ErrorAction SilentlyContinue
if (-not $volume -or $volume.FileSystemLabel -ne 'LHC-CACHE' -or $volume.FileSystem -ne 'NTFS') {
    Get-Disk | Format-Table Number, FriendlyName, Size, PartitionStyle
    throw 'Attach the cache disk and pass its explicit -CacheDiskNumber; D: must be NTFS labelled LHC-CACHE.'
}
New-Item -ItemType Directory -Force -Path 'D:\lhc-e2e\src', 'D:\lhc-e2e\out' | Out-Null
icacls 'D:\lhc-e2e' /grant "${User}:(OI)(CI)F" | Out-Null

Write-Host 'Scheduled task LHC-E2E...'
$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument `
    '-NoProfile -ExecutionPolicy Bypass -File D:\lhc-e2e\src\apps\slint-shell\scripts\vm\windows-guest.ps1'
$principal = New-ScheduledTaskPrincipal -UserId $User -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -Priority 4 -ExecutionTimeLimit (New-TimeSpan -Hours 3) `
    -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
Register-ScheduledTask -TaskName 'LHC-E2E' -Action $action -Principal $principal `
    -Settings $settings -Force | Out-Null

$cargo = "C:\Users\$User\.cargo\bin\cargo.exe"
if (-not (Test-Path $cargo)) {
    Write-Warning "Rust is not installed for $User; see docs/testing/windows.md, Toolchain installation."
}
Write-Host 'Prepared. Reboot once to confirm autologon, then shut down and freeze the disk.'
