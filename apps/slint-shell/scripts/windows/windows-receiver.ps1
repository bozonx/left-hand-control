param(
    [string]$Output = (Join-Path (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path 'windows-runtime\receiver'),
    [string]$InputLocale
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes
$script:receiverScript = $PSCommandPath
Add-Type -Path (Join-Path $PSScriptRoot 'windows-key-trace.cs') -ReferencedAssemblies 'System.dll', 'System.Core.dll', 'System.Windows.Forms.dll'
function Set-ReceiverLanguage([string]$Locale) {
    $language = [System.Windows.Forms.InputLanguage]::InstalledInputLanguages |
        Where-Object { $_.Culture.Name -eq $Locale } | Select-Object -First 1
    if (-not $language) {
        # Load the built-in test layout while the receiver owns keyboard focus.
        $identifier = switch ($Locale) {
            'en-US' { '00000409' }
            'ru-RU' { '00000419' }
            default { throw "Unsupported test input language: $Locale" }
        }
        $layout = [LhcKeyTrace]::LoadKeyboardLayoutW($identifier, 1)
        if ($layout -eq [IntPtr]::Zero) { throw "Could not load input language: $Locale" }
        $language = [System.Windows.Forms.InputLanguage]::InstalledInputLanguages |
            Where-Object { $_.Handle -eq $layout } | Select-Object -First 1
        if (-not $language) { throw "Loaded input language is unavailable: $Locale" }
    }
    if ($language.Culture.Name -ne $Locale) { throw "Unexpected input language: $($language.Culture.Name), expected $Locale" }
    [System.Windows.Forms.InputLanguage]::CurrentInputLanguage = $language
}
if ($InputLocale) {
    Set-ReceiverLanguage $InputLocale
}

New-Item -ItemType Directory -Force -Path $Output | Out-Null
$eventsPath = Join-Path $Output 'events.jsonl'
$snapshotPath = Join-Path $Output 'snapshot.json'
Remove-Item $eventsPath -ErrorAction SilentlyContinue
Remove-Item $snapshotPath -ErrorAction SilentlyContinue

$form = New-Object System.Windows.Forms.Form
$form.Text = 'LHC Unicode Receiver'
$form.StartPosition = 'CenterScreen'
$form.ClientSize = New-Object System.Drawing.Size(760, 420)
$form.Font = New-Object System.Drawing.Font('Segoe UI', 12)

$status = New-Object System.Windows.Forms.Label
$status.Dock = 'Top'
$status.Height = 42
$status.Padding = New-Object System.Windows.Forms.Padding(8)
$status.Text = 'Focus this editor, invoke a popup, then select an item.'

$editor = New-Object System.Windows.Forms.TextBox
$editor.Name = 'ReceiverEditor'
$editor.Dock = 'Fill'
$editor.Multiline = $true
$editor.AcceptsReturn = $true
$editor.AcceptsTab = $true
$editor.ScrollBars = 'Both'
$editor.WordWrap = $false

$form.Controls.Add($editor)
$form.Controls.Add($status)
$script:formActive = $false
$script:editorFocused = $false
$script:lastCommand = $null
$script:receiverElevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$script:hideOnInput = $false

function Write-ReceiverState {
    param([string]$Event, $KeyEvent = $null)

    $text = $editor.Text
    $units = [System.Collections.Generic.List[int]]::new()
    foreach ($unit in $text.ToCharArray()) {
        $units.Add([int]$unit)
    }
    $codePoints = [System.Collections.Generic.List[int]]::new()
    for ($index = 0; $index -lt $text.Length; $index++) {
        $first = $text[$index]
        if ([char]::IsHighSurrogate($first) -and $index + 1 -lt $text.Length -and [char]::IsLowSurrogate($text[$index + 1])) {
            $codePoints.Add([char]::ConvertToUtf32($first, $text[$index + 1]))
            $index++
        } else {
            $codePoints.Add([int]$first)
        }
    }
    $state = [ordered]@{
        timestamp = [DateTimeOffset]::Now.ToString('o')
        event = $Event
        elevated = $script:receiverElevated
        input_locale = [System.Windows.Forms.InputLanguage]::CurrentInputLanguage.Culture.Name
        form_active = ([LhcKeyTrace]::GetForegroundWindow() -eq $form.Handle)
        editor_focused = $script:editorFocused
        modifiers_down = @(160, 161, 162, 163, 164, 165, 91, 92 | Where-Object { [LhcKeyTrace]::IsKeyDown($_) })
        key_code = if ($KeyEvent) { [int]$KeyEvent.KeyCode } else { $null }
        modifiers = if ($KeyEvent) { [int]$KeyEvent.Modifiers } else { $null }
        text = $text
        utf16 = $units
        code_points = $codePoints
    }
    $json = $state | ConvertTo-Json -Compress
    [System.IO.File]::WriteAllText("$snapshotPath.tmp", $json, [System.Text.UTF8Encoding]::new($false))
    if (Test-Path $snapshotPath) {
        # PowerShell 5.1 otherwise coerces $null to an invalid empty backup path.
        [System.IO.File]::Replace("$snapshotPath.tmp", $snapshotPath, [NullString]::Value)
    } else {
        [System.IO.File]::Move("$snapshotPath.tmp", $snapshotPath)
    }
    [System.IO.File]::AppendAllText($eventsPath, "$json`n", [System.Text.UTF8Encoding]::new($false))
    $status.Text = "Event: $Event | UTF-16 units: $($units.Count) | Code points: $($state.code_points.Count)"
}

$form.Add_Shown({
    $editor.Focus() | Out-Null
    $script:editorFocused = $editor.Focused
    Write-ReceiverState 'shown'
})
$form.Add_Activated({
    $script:formActive = $true
    $script:editorFocused = $editor.Focused
    Write-ReceiverState 'form-activated'
})
$form.Add_Deactivate({
    $script:formActive = $false
    $script:editorFocused = $false
    Write-ReceiverState 'form-deactivated'
})
$editor.Add_Enter({
    $script:editorFocused = $true
    Write-ReceiverState 'editor-focus'
})
$editor.Add_Leave({
    $script:editorFocused = $false
    Write-ReceiverState 'editor-blur'
})
$editor.Add_KeyDown({ param($sender, $eventArgs) Write-ReceiverState 'key-down' $eventArgs })
$editor.Add_KeyUp({ param($sender, $eventArgs) Write-ReceiverState 'key-up' $eventArgs })
$editor.Add_TextChanged({
    Write-ReceiverState 'text-changed'
    if ($script:hideOnInput -and $editor.Text.Length -gt 0) {
        $script:hideOnInput = $false
        $form.Hide()
    }
})
$form.Add_FormClosed({ Write-ReceiverState 'closed' })

function Write-NotepadState {
    if (-not $script:notepad -or $script:notepad.HasExited) { return }
    try {
        $script:notepad.Refresh()
        $handle = $script:notepad.MainWindowHandle
        $element = [Windows.Automation.AutomationElement]::FromHandle($handle)
        $condition = [Windows.Automation.OrCondition]::new(
            [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Document),
            [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Edit))
        $control = $element.FindFirst([Windows.Automation.TreeScope]::Descendants, $condition)
        if (-not $control) { throw 'No Notepad text control' }
        $pattern = $control.GetCurrentPattern([Windows.Automation.TextPattern]::Pattern)
        $value = @{ pid=$script:notepad.Id; text=$pattern.DocumentRange.GetText(-1); active=([LhcKeyTrace]::GetForegroundWindow() -eq $handle) }
    } catch { $value = @{ error="$_"; active=$false } }
    $path = Join-Path $Output 'notepad.json'
    [IO.File]::WriteAllText("$path.tmp", ($value | ConvertTo-Json -Compress), [Text.UTF8Encoding]::new($false))
    if (Test-Path $path) { [IO.File]::Replace("$path.tmp", $path, [NullString]::Value) } else { [IO.File]::Move("$path.tmp", $path) }
}

# Host control changes the receiver itself; keyboard scenarios still use QMP.
$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 50
$timer.Add_Tick({
    Write-NotepadState
    if ($script:suspendPending -and (Get-Date) -ge $script:suspendAt) {
        $script:suspendPending = $false
        if (-not [Windows.Forms.Application]::SetSuspendState('Suspend', $false, $false)) {
            [IO.File]::WriteAllText((Join-Path $Output 'suspend-error.txt'), 'SetSuspendState failed')
        }
    }
    $raw = [LhcKeyTrace]::Drain()
    if ($raw.Count) {
        [IO.File]::AppendAllText((Join-Path $Output 'raw-keys.jsonl'), (($raw -join "`n") + "`n"), [Text.UTF8Encoding]::new($false))
    }
    $path = Join-Path $Output 'command.json'
    if (-not (Test-Path $path)) { return }
    try {
        $stream = [IO.File]::Open($path, 'Open', 'Read', ([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
        $reader = [IO.StreamReader]::new($stream, [Text.Encoding]::UTF8)
        try { $rawCommand = $reader.ReadToEnd() } finally { $reader.Dispose() }
        $command = $rawCommand | ConvertFrom-Json
    } catch { return }
    if ($command.id -eq $script:lastCommand) { return }
    $script:lastCommand = $command.id
    try {
        switch ($command.op) {
            'freeze-crash-tree' {
                $directory = Split-Path $Output
                $owner = [int](Get-Content (Join-Path $directory 'mapper\crash\ready') -Raw)
                $tree = @($owner)
                $processes = @(Get-CimInstance Win32_Process)
                do {
                    $children = @($processes | Where-Object { $_.ParentProcessId -in $tree -and $_.ProcessId -notin $tree } | ForEach-Object { [int]$_.ProcessId })
                    $tree += $children
                } while ($children.Count)
                if (-not $command.guard_pid -or [int]$command.guard_pid -in $tree) { throw 'Input guard must survive outside the live owner tree' }
                foreach ($pidToFreeze in $tree) {
                    if ([LhcKeyTrace]::SuspendProcess($pidToFreeze) -ne 0) { throw "Cannot suspend owner tree PID $pidToFreeze" }
                }
            }
            'notepad' {
                $before = @(Get-Process notepad -ErrorAction SilentlyContinue | ForEach-Object Id)
                Start-Process notepad
                $deadline = (Get-Date).AddSeconds(30)
                do {
                    $script:notepad = Get-Process notepad -ErrorAction SilentlyContinue | Where-Object {
                        $_.Id -notin $before -and $_.SessionId -eq [Diagnostics.Process]::GetCurrentProcess().SessionId -and $_.MainWindowHandle -ne 0
                    } | Select-Object -First 1
                    if ((Get-Date) -gt $deadline) { throw 'No owned interactive Notepad' }
                    Start-Sleep -Milliseconds 100
                } until ($script:notepad)
            }
            'notepad-close' {
                if ($script:notepad -and -not $script:notepad.HasExited) { $script:notepad.Kill(); $script:notepad.WaitForExit() }
            }
            'elevate' {
                $elevatedOutput = Join-Path (Split-Path $Output) 'elevated'
                Start-Process powershell -Verb RunAs -ArgumentList @('-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-File', $script:receiverScript, '-Output', $elevatedOutput)
            }
            'close' { $script:closePending = $true }
            'session-state' { }
            'lock' {
                if (-not [LhcKeyTrace]::LockWorkStation()) { throw 'LockWorkStation failed' }
            }
            'suspend' { $script:suspendAt = (Get-Date).AddSeconds(2); $script:suspendPending = $true }
            'trace' { [LhcKeyTrace]::Stop(); [LhcKeyTrace]::Start() }
            'reset' { $script:hideOnInput = $false; $editor.Clear() }
            'hide-on-input' { $script:hideOnInput = $true }
            'focus' { $form.Activate(); $editor.Focus() | Out-Null }
            'blur' { $status.Focus() | Out-Null; $form.Hide() }
            'locale' {
                Set-ReceiverLanguage $command.locale
            }
            'show' {
                $form.Show()
                $form.TopMost = $true
                $form.Activate()
                $editor.Focus() | Out-Null
            }
            'unpin' { $form.TopMost = $false }
            default { throw "Unknown receiver operation: $($command.op)" }
        }
        Write-ReceiverState 'command'
        $response = @{ id = $command.id; ok = $true }
        if ($command.op -eq 'session-state') { $response.locked = [LhcKeyTrace]::IsSessionLocked() }
    } catch { $response = @{ id = $command.id; ok = $false; error = "$_" } }
    $json = $response | ConvertTo-Json -Compress
    [IO.File]::WriteAllText((Join-Path $Output 'response.json'), $json, [Text.UTF8Encoding]::new($false))
    if ($script:closePending) { $form.Close() }

})
$timer.Start()
try { [System.Windows.Forms.Application]::Run($form) } finally { $timer.Stop(); [LhcKeyTrace]::Stop(); $timer.Dispose() }
