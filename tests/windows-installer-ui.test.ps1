$ErrorActionPreference = 'Stop'
$sourcePath = Join-Path $PSScriptRoot '..\src\installer\windows_wizard.ps1'
$source = Get-Content -LiteralPath $sourcePath -Raw -Encoding UTF8
$tokens = $null
$errors = $null
[System.Management.Automation.Language.Parser]::ParseInput($source, $sourcePath, [ref]$tokens, [ref]$errors) | Out-Null
if ($errors.Count) { throw "Installer script syntax: $($errors[0].Message)" }

$end = $source.LastIndexOf('Show-Step 0')
if ($end -lt 0) { throw 'Installer entry point not found' }
$env:NIKI_DEFAULT_DIR = Join-Path $env:TEMP 'Niki-Installer-Test'
$env:NIKI_DESKTOP_DEFAULT = '0'
$env:NIKI_VERSION = 'test'
$env:NIKI_SETUP_EXE = Join-Path $env:TEMP 'Niki_Setup.exe'
Invoke-Expression $source.Substring(0, $end)

if ($script:pages.Count -ne 7) { throw 'Installer pages are missing' }
if ($pathInput.Text -ne $env:NIKI_DEFAULT_DIR) { throw 'Default installation path is missing' }
if ($desktopCheck.Checked) { throw 'Desktop shortcut preference was ignored' }
Show-Step 1
if ($script:step -ne 1) { throw 'Location step is unavailable' }
Show-Step 2
if ($script:step -ne 2) { throw 'Shortcut step is unavailable' }
$desktopCheck.Checked = $false
Show-Step 3
$withoutShortcut = $reviewShortcut.Text
$desktopCheck.Checked = $true
Show-Step 3
if (-not $withoutShortcut -or $reviewShortcut.Text -eq $withoutShortcut) { throw 'Shortcut choice is absent from review' }
if ($env:NIKI_INSTALLER_PREVIEW_DIR) {
    $form.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
    $form.Location = New-Object System.Drawing.Point(-2000, -2000)
    $form.ShowInTaskbar = $false
    $form.Show()
    foreach ($pageNumber in 0..3) {
        Show-Step $pageNumber
        [System.Windows.Forms.Application]::DoEvents()
        $image = New-Object System.Drawing.Bitmap($form.Width, $form.Height)
        $form.DrawToBitmap($image, (New-Object System.Drawing.Rectangle(0, 0, $form.Width, $form.Height)))
        $image.Save((Join-Path $env:NIKI_INSTALLER_PREVIEW_DIR "install-step-$pageNumber.png"))
        $image.Dispose()
    }
    $form.Hide()
}
$script:timer.Dispose()
$form.Dispose()
Write-Output 'Windows installer wizard layout and choices passed'
