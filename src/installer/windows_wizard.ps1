$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()

$green = [System.Drawing.ColorTranslator]::FromHtml('#315e42')
$ink = [System.Drawing.ColorTranslator]::FromHtml('#25352a')
$muted = [System.Drawing.ColorTranslator]::FromHtml('#718475')
$surface = [System.Drawing.ColorTranslator]::FromHtml('#f8f8f5')
$border = [System.Drawing.ColorTranslator]::FromHtml('#dfe6dd')
$script:step = 0
$script:completed = $false
$script:installProcess = $null

function Add-Text($parent, $text, $x, $y, $width, $height, $size = 10, $bold = $false) {
    $label = New-Object System.Windows.Forms.Label
    $label.Text = $text
    $label.Location = New-Object System.Drawing.Point($x, $y)
    $label.Size = New-Object System.Drawing.Size($width, $height)
    $fontStyle = if ($bold) { [System.Drawing.FontStyle]::Bold } else { [System.Drawing.FontStyle]::Regular }
    $label.Font = New-Object System.Drawing.Font('Segoe UI', [single]$size, $fontStyle)
    $label.ForeColor = $ink
    $parent.Controls.Add($label)
    return $label
}

function New-Page {
    $page = New-Object System.Windows.Forms.Panel
    $page.Location = New-Object System.Drawing.Point(34, 108)
    $page.Size = New-Object System.Drawing.Size(572, 226)
    $page.Visible = $false
    $form.Controls.Add($page)
    return $page
}

function New-Button($text, $x, $primary = $false) {
    $button = New-Object System.Windows.Forms.Button
    $button.Text = $text
    $button.Location = New-Object System.Drawing.Point($x, 371)
    $button.Size = New-Object System.Drawing.Size(104, 36)
    $button.FlatStyle = [System.Windows.Forms.FlatStyle]::Flat
    $button.Font = New-Object System.Drawing.Font('Segoe UI', [single]9, [System.Drawing.FontStyle]::Bold)
    $button.BackColor = $(if ($primary) { $green } else { [System.Drawing.Color]::White })
    $button.ForeColor = $(if ($primary) { [System.Drawing.Color]::White } else { $ink })
    $button.FlatAppearance.BorderColor = $(if ($primary) { $green } else { $border })
    $form.Controls.Add($button)
    return $button
}

$form = New-Object System.Windows.Forms.Form
$form.Text = 'Niki 설치'
$form.ClientSize = New-Object System.Drawing.Size(640, 428)
$form.MinimumSize = $form.Size
$form.MaximumSize = $form.Size
$form.StartPosition = [System.Windows.Forms.FormStartPosition]::CenterScreen
$form.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::FixedDialog
$form.MaximizeBox = $false
$form.MinimizeBox = $false
$form.BackColor = $surface
$form.Font = New-Object System.Drawing.Font('Segoe UI', [single]10)
try { $form.Icon = [System.Drawing.Icon]::ExtractAssociatedIcon($env:NIKI_SETUP_EXE) } catch { }

$header = New-Object System.Windows.Forms.Panel
$header.Location = New-Object System.Drawing.Point(0, 0)
$header.Size = New-Object System.Drawing.Size(640, 86)
$header.BackColor = [System.Drawing.Color]::White
$form.Controls.Add($header)
$brand = Add-Text $header 'Niki' 34 24 250 35 20 $true
$brand.ForeColor = $green
$script:stepLabel = Add-Text $header '' 516 33 90 24 9 $true
$script:stepLabel.ForeColor = $muted
$divider = New-Object System.Windows.Forms.Panel
$divider.Location = New-Object System.Drawing.Point(34, 352)
$divider.Size = New-Object System.Drawing.Size(572, 1)
$divider.BackColor = $border
$form.Controls.Add($divider)

$welcome = New-Page
$logo = Add-Text $welcome 'N' 0 24 64 64 26 $true
$logo.BackColor = $green
$logo.ForeColor = [System.Drawing.Color]::White
$logo.TextAlign = [System.Drawing.ContentAlignment]::MiddleCenter
[void](Add-Text $welcome 'Niki를 설치합니다' 84 20 440 38 19 $true)
[void](Add-Text $welcome "버전 $env:NIKI_VERSION" 85 61 440 24 10)
[void](Add-Text $welcome '설치 위치와 바탕화면 바로가기를 순서대로 선택할 수 있습니다.' 0 125 560 26 10)
[void](Add-Text $welcome '기존 일기와 설정은 설치 과정에서 삭제되지 않습니다.' 0 155 560 26 10)

$location = New-Page
[void](Add-Text $location '설치 위치 선택' 0 0 560 38 18 $true)
[void](Add-Text $location 'Niki를 설치할 폴더를 선택하세요.' 0 48 560 25 10)
$pathInput = New-Object System.Windows.Forms.TextBox
$pathInput.Location = New-Object System.Drawing.Point(0, 91)
$pathInput.Size = New-Object System.Drawing.Size(446, 32)
$pathInput.Text = $env:NIKI_DEFAULT_DIR
$pathInput.Font = New-Object System.Drawing.Font('Segoe UI', [single]10)
$location.Controls.Add($pathInput)
$browse = New-Object System.Windows.Forms.Button
$browse.Text = '찾아보기'
$browse.Location = New-Object System.Drawing.Point(458, 89)
$browse.Size = New-Object System.Drawing.Size(104, 31)
$browse.FlatStyle = [System.Windows.Forms.FlatStyle]::Flat
$browse.FlatAppearance.BorderColor = $border
$browse.BackColor = [System.Drawing.Color]::White
$location.Controls.Add($browse)
$hint = Add-Text $location '선택한 폴더에 직접 설치합니다. 드라이브의 최상위 폴더는 선택할 수 없습니다.' 0 143 560 42 9
$hint.ForeColor = $muted
$browse.Add_Click({
    $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
    $dialog.Description = 'Niki를 설치할 폴더를 선택하세요.'
    $dialog.ShowNewFolderButton = $true
    $initial = $pathInput.Text
    while ($initial -and -not [System.IO.Directory]::Exists($initial)) {
        $initial = [System.IO.Path]::GetDirectoryName($initial)
    }
    if ($initial) { $dialog.SelectedPath = $initial }
    if ($dialog.ShowDialog($form) -eq [System.Windows.Forms.DialogResult]::OK) {
        $pathInput.Text = $dialog.SelectedPath
    }
    $dialog.Dispose()
})

$shortcuts = New-Page
[void](Add-Text $shortcuts '바로가기 선택' 0 0 560 38 18 $true)
[void](Add-Text $shortcuts '자주 사용하는 위치에서 Niki를 열 수 있습니다.' 0 48 560 25 10)
$desktopCheck = New-Object System.Windows.Forms.CheckBox
$desktopCheck.Text = '바탕화면에 Niki 바로가기 만들기'
$desktopCheck.Location = New-Object System.Drawing.Point(0, 91)
$desktopCheck.Size = New-Object System.Drawing.Size(560, 33)
$desktopCheck.Checked = $env:NIKI_DESKTOP_DEFAULT -ne '0'
$desktopCheck.ForeColor = $ink
$shortcuts.Controls.Add($desktopCheck)
$shortcutHint = Add-Text $shortcuts '시작 메뉴에는 항상 바로가기를 추가합니다.' 24 133 530 28 9
$shortcutHint.ForeColor = $muted

$review = New-Page
[void](Add-Text $review '설치 내용 확인' 0 0 560 38 18 $true)
[void](Add-Text $review '아래 설정으로 Niki를 설치합니다.' 0 47 560 25 10)
$reviewPath = Add-Text $review '' 0 93 560 51 10 $true
$reviewShortcut = Add-Text $review '' 0 151 560 28 10

$progress = New-Page
[void](Add-Text $progress '설치 중' 0 0 560 38 18 $true)
[void](Add-Text $progress '프로그램 파일과 바로가기를 준비하고 있습니다. 잠시만 기다려 주세요.' 0 50 560 29 10)
$progressBar = New-Object System.Windows.Forms.ProgressBar
$progressBar.Location = New-Object System.Drawing.Point(0, 111)
$progressBar.Size = New-Object System.Drawing.Size(562, 15)
$progressBar.Style = [System.Windows.Forms.ProgressBarStyle]::Marquee
$progressBar.MarqueeAnimationSpeed = 30
$progress.Controls.Add($progressBar)

$finished = New-Page
[void](Add-Text $finished '설치가 완료되었습니다' 0 0 560 38 18 $true)
[void](Add-Text $finished 'Niki를 시작하거나 설치 마법사를 닫을 수 있습니다.' 0 50 560 28 10)
$finishPath = Add-Text $finished '' 0 105 560 54 9
$finishPath.ForeColor = $muted

$failed = New-Page
$failureTitle = Add-Text $failed '설치를 완료하지 못했습니다' 0 0 560 38 18 $true
$failureTitle.ForeColor = [System.Drawing.ColorTranslator]::FromHtml('#ad5d59')
$failureText = Add-Text $failed '' 0 51 560 125 10

$script:pages = @($welcome, $location, $shortcuts, $review, $progress, $finished, $failed)
$back = New-Button '이전' 274
$next = New-Button '다음' 384 $true
$cancel = New-Button '취소' 494
$form.AcceptButton = $next
$form.CancelButton = $cancel

function Show-Step($index) {
    $script:step = $index
    for ($i = 0; $i -lt $script:pages.Count; $i++) {
        $script:pages[$i].Visible = $i -eq $index
    }
    $script:stepLabel.Text = if ($index -lt 4) { "$(($index + 1)) / 4" } elseif ($index -eq 4) { '설치 중' } else { '완료' }
    $back.Visible = $index -ge 1 -and $index -le 3
    $next.Visible = $index -ne 4
    $next.Text = switch ($index) { 3 { '설치' } 5 { 'Niki 시작' } 6 { '다시 시도' } default { '다음' } }
    $cancel.Text = if ($index -eq 5) { '닫기' } else { '취소' }
    $cancel.Enabled = $index -ne 4
    $form.ControlBox = $index -ne 4
    if ($index -eq 3) {
        $reviewPath.Text = "설치 위치`r`n$($pathInput.Text)"
        $reviewShortcut.Text = if ($desktopCheck.Checked) { '바탕화면 바로가기: 만들기' } else { '바탕화면 바로가기: 만들지 않기' }
    }
    if ($index -eq 5) { $finishPath.Text = "설치 위치: $($pathInput.Text)" }
}

$script:timer = New-Object System.Windows.Forms.Timer
$script:timer.Interval = 200
$script:timer.Add_Tick({
    if (-not $script:installProcess -or -not $script:installProcess.HasExited) { return }
    $script:timer.Stop()
    $exitCode = $script:installProcess.ExitCode
    $detail = $script:installProcess.StandardError.ReadToEnd().Trim()
    $script:installProcess.Dispose()
    $script:installProcess = $null
    if ($exitCode -eq 0) {
        Show-Step 5
    } else {
        $failureText.Text = if ($detail) { $detail } else { '설치 파일을 확인하고 다시 시도해 주세요.' }
        Show-Step 6
    }
})

function Start-Installation {
    Show-Step 4
    try {
        $start = New-Object System.Diagnostics.ProcessStartInfo
        $start.FileName = $env:NIKI_SETUP_EXE
        $start.Arguments = '--install --silent'
        $start.UseShellExecute = $false
        $start.CreateNoWindow = $true
        $start.RedirectStandardError = $true
        $start.EnvironmentVariables['NIKI_INSTALL_FROM_WIZARD'] = '1'
        $start.EnvironmentVariables['NIKI_INSTALL_DIR'] = $pathInput.Text
        $start.EnvironmentVariables['NIKI_DESKTOP_SHORTCUT'] = if ($desktopCheck.Checked) { '1' } else { '0' }
        $start.EnvironmentVariables['NIKI_SKIP_LAUNCH'] = '1'
        $script:installProcess = [System.Diagnostics.Process]::Start($start)
        if (-not $script:installProcess) { throw '설치 작업을 시작하지 못했습니다.' }
        $script:timer.Start()
    } catch {
        $failureText.Text = $_.Exception.Message
        Show-Step 6
    }
}

$back.Add_Click({ Show-Step ($script:step - 1) })
$next.Add_Click({
    switch ($script:step) {
        0 { Show-Step 1 }
        1 {
            try {
                $candidate = $pathInput.Text.Trim()
                if ($candidate -notmatch '^[A-Za-z]:\\') { throw '로컬 드라이브의 절대 경로를 입력해 주세요.' }
                $full = [System.IO.Path]::GetFullPath($candidate)
                if ($full.TrimEnd('\') -ieq [System.IO.Path]::GetPathRoot($full).TrimEnd('\')) {
                    throw '드라이브의 최상위 폴더는 설치 위치로 사용할 수 없습니다.'
                }
                $pathInput.Text = $full
                Show-Step 2
            } catch {
                [System.Windows.Forms.MessageBox]::Show($form, $_.Exception.Message, '설치 위치 확인', 'OK', 'Warning') | Out-Null
            }
        }
        2 { Show-Step 3 }
        3 { Start-Installation }
        5 {
            $script:completed = $true
            $installedExe = [System.IO.Path]::Combine($pathInput.Text, 'Niki.exe')
            try { [System.Diagnostics.Process]::Start($installedExe) | Out-Null }
            catch { [System.Windows.Forms.MessageBox]::Show($form, $_.Exception.Message, 'Niki 실행 오류', 'OK', 'Error') | Out-Null }
            $form.Close()
        }
        6 { Show-Step 3 }
    }
})
$cancel.Add_Click({
    if ($script:step -eq 5) { $script:completed = $true }
    $form.Close()
})
$form.Add_FormClosing({ if ($script:step -eq 4) { $_.Cancel = $true } })

Show-Step 0
[void]$form.ShowDialog()
$script:timer.Dispose()
$form.Dispose()
if ($script:completed) { exit 0 }
exit 2
