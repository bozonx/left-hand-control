# Syntax-only check; safe in CI and without an interactive desktop.
$ErrorActionPreference = 'Stop'
$count = 0
foreach ($file in Get-ChildItem $PSScriptRoot -Recurse -File -Filter '*.ps1') {
    $tokens = $null
    $parseErrors = $null
    [System.Management.Automation.Language.Parser]::ParseFile($file.FullName, [ref]$tokens, [ref]$parseErrors) | Out-Null
    if ($parseErrors.Count) {
        $messages = @($parseErrors | ForEach-Object { "$($_.Extent.StartLineNumber): $($_.Message)" }) -join '; '
        throw "$($file.FullName): $messages"
    }
    $count++
}
Write-Host "WINDOWS SCRIPT SYNTAX PASSED: $count files"
