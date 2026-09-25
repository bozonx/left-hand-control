param(
    [string]$Output = (Join-Path (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path 'windows-runtime\receiver')
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

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

function Write-ReceiverState {
    param([string]$Event)

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
        form_active = $script:formActive
        editor_focused = $script:editorFocused
        text = $text
        utf16 = $units
        code_points = $codePoints
    }
    $json = $state | ConvertTo-Json -Compress
    [System.IO.File]::WriteAllText($snapshotPath, $json, [System.Text.UTF8Encoding]::new($false))
    [System.IO.File]::AppendAllText($eventsPath, "$json`n", [System.Text.UTF8Encoding]::new($false))
    $status.Text = "Event: $Event | UTF-16 units: $($units.Count) | Code points: $($state.code_points.Count)"
}

$form.Add_Shown({
    $script:formActive = $true
    $script:editorFocused = $true
    $editor.Focus()
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
$editor.Add_TextChanged({ Write-ReceiverState 'text-changed' })
$form.Add_FormClosed({ Write-ReceiverState 'closed' })

[System.Windows.Forms.Application]::Run($form)
