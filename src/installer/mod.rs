use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(target_os = "windows")]
const UNINSTALL_REG_KEY: &str =
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\app.niki.journal";
#[cfg(target_os = "windows")]
const APP_ICON_ICO: &[u8] = include_bytes!("../../icons/icon.ico");
#[cfg(target_os = "macos")]
const APP_ICON_ICNS: &[u8] = include_bytes!("../../icons/icon.icns");

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(target_os = "windows")]
pub fn install_dir() -> Result<PathBuf, String> {
    let local_app_data = env::var_os("LOCALAPPDATA")
        .ok_or("LOCALAPPDATA 환경 변수를 찾을 수 없습니다.")?;
    Ok(PathBuf::from(local_app_data).join("Programs").join("Niki"))
}

#[cfg(target_os = "macos")]
pub fn install_dir() -> Result<PathBuf, String> {
    let sys_apps = PathBuf::from("/Applications");
    let sys_bundle = sys_apps.join("Niki.app");
    if sys_bundle.exists() || is_dir_writable(&sys_apps) {
        return Ok(sys_bundle);
    }
    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home).join("Applications").join("Niki.app"));
    }
    Ok(sys_bundle)
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn install_dir() -> Result<PathBuf, String> {
    let home = env::var_os("HOME").ok_or("HOME 환경 변수를 찾을 수 없습니다.")?;
    Ok(PathBuf::from(home).join(".local").join("bin"))
}

#[cfg(target_os = "macos")]
fn is_dir_writable(dir: &Path) -> bool {
    fs::metadata(dir)
        .map(|m| !m.permissions().readonly())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
pub fn installed_exe_path() -> Result<PathBuf, String> {
    Ok(install_dir()?.join("Niki.exe"))
}

#[cfg(target_os = "macos")]
pub fn installed_exe_path() -> Result<PathBuf, String> {
    install_dir()
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn installed_exe_path() -> Result<PathBuf, String> {
    Ok(install_dir()?.join("niki_journal"))
}

#[cfg(target_os = "windows")]
pub fn is_running_from_installed_location() -> bool {
    let Ok(cur) = env::current_exe() else {
        return false;
    };
    let Ok(inst) = installed_exe_path() else {
        return false;
    };
    normalize_path(&cur) == normalize_path(&inst)
}

#[cfg(target_os = "macos")]
pub fn is_running_from_installed_location() -> bool {
    let Ok(cur) = env::current_exe() else {
        return false;
    };
    let cur_str = cur.to_string_lossy();
    if cur_str.starts_with("/Applications/Niki.app/") {
        return true;
    }
    if let Ok(inst) = install_dir() {
        let prefix = format!("{}/", inst.to_string_lossy());
        if cur_str.starts_with(&prefix) {
            return true;
        }
    }
    false
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn is_running_from_installed_location() -> bool {
    let Ok(cur) = env::current_exe() else {
        return false;
    };
    let Ok(inst) = installed_exe_path() else {
        return false;
    };
    cur == inst
}

#[cfg(target_os = "windows")]
pub fn is_installed_on_pc() -> bool {
    installed_exe_path().is_ok_and(|p| p.is_file())
}

#[cfg(target_os = "macos")]
pub fn is_installed_on_pc() -> bool {
    let check_bundle = |bundle: &Path| -> bool {
        bundle.join("Contents/MacOS/Niki").is_file()
            || bundle.join("Contents/MacOS/niki_journal").is_file()
    };
    if check_bundle(Path::new("/Applications/Niki.app")) {
        return true;
    }
    install_dir().is_ok_and(|p| check_bundle(&p))
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn is_installed_on_pc() -> bool {
    installed_exe_path().is_ok_and(|p| p.is_file())
}

#[cfg(target_os = "windows")]
fn normalize_path(p: &Path) -> String {
    p.to_string_lossy()
        .replace('/', "\\")
        .trim_start_matches(r"\\?\")
        .to_ascii_lowercase()
}

pub enum StartupMode {
    NormalApp,
    Install { silent: bool },
    Uninstall,
}

pub fn detect_startup_mode() -> StartupMode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a.eq_ignore_ascii_case("--uninstall")) {
        return StartupMode::Uninstall;
    }
    let silent = args
        .iter()
        .any(|a| a.eq_ignore_ascii_case("--silent") || a.eq_ignore_ascii_case("/S"));
    if silent || args.iter().any(|a| a.eq_ignore_ascii_case("--install")) {
        return StartupMode::Install { silent };
    }
    #[cfg(target_os = "windows")]
    if let Ok(exe) = env::current_exe()
        && let Some(stem) = exe.file_stem().and_then(|s| s.to_str())
    {
        let lower = stem.to_ascii_lowercase();
        if lower.contains("setup") || lower.contains("install") || lower.starts_with("niki-update-")
        {
            return StartupMode::Install { silent: false };
        }
    }
    StartupMode::NormalApp
}

pub fn ensure_release_setup_binary() {
    #[cfg(target_os = "windows")]
    {
        let Ok(cur) = env::current_exe() else {
            return;
        };
        let Some(name) = cur.file_name().and_then(|s| s.to_str()) else {
            return;
        };
        if !name.eq_ignore_ascii_case("niki_journal.exe") {
            return;
        }
        if let Some(parent) = cur.parent() {
            let setup_path = parent.join("Niki_Setup.exe");
            let should_copy = match (fs::metadata(&cur), fs::metadata(&setup_path)) {
                (Ok(src_m), Ok(dst_m)) => {
                    src_m.len() != dst_m.len()
                        || src_m.modified().ok() > dst_m.modified().ok()
                }
                (Ok(_), Err(_)) => true,
                _ => false,
            };
            if should_copy {
                let _ = fs::copy(&cur, &setup_path);
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let Ok(cur) = env::current_exe() else {
            return;
        };
        let Some(name) = cur.file_name().and_then(|s| s.to_str()) else {
            return;
        };
        if name != "niki_journal" {
            return;
        }
        // Skip if already running inside a .app bundle
        if cur.to_string_lossy().contains(".app/Contents/MacOS/") {
            return;
        }
        if let Some(parent) = cur.parent() {
            let app_bundle = parent.join("Niki.app");
            let bundle_bin = app_bundle.join("Contents/MacOS/Niki");
            let should_build = match (fs::metadata(&cur), fs::metadata(&bundle_bin)) {
                (Ok(src_m), Ok(dst_m)) => {
                    src_m.len() != dst_m.len()
                        || src_m.modified().ok() > dst_m.modified().ok()
                }
                (Ok(_), Err(_)) => true,
                _ => false,
            };
            if should_build {
                let _ = create_macos_app_bundle(&cur, &app_bundle);
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub fn create_macos_app_bundle(src_binary: &Path, bundle_path: &Path) -> Result<PathBuf, String> {
    let contents_dir = bundle_path.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");

    fs::create_dir_all(&macos_dir)
        .map_err(|e| format!("앱 번들 MacOS 디렉토리 생성 실패: {e}"))?;
    fs::create_dir_all(&resources_dir)
        .map_err(|e| format!("앱 번들 Resources 디렉토리 생성 실패: {e}"))?;

    let target_bin = macos_dir.join("Niki");
    if src_binary != target_bin {
        fs::copy(src_binary, &target_bin)
            .map_err(|e| format!("실행 파일 복사 실패: {e}"))?;
    }
    let mut perms = fs::metadata(&target_bin)
        .map_err(|e| format!("실행 파일 권한 조회 실패: {e}"))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&target_bin, perms)
        .map_err(|e| format!("실행 파일 권한 설정 실패: {e}"))?;

    let icon_path = resources_dir.join("icon.icns");
    fs::write(&icon_path, APP_ICON_ICNS)
        .map_err(|e| format!("아이콘 파일 기록 실패: {e}"))?;

    let info_plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>ko</string>
  <key>CFBundleDisplayName</key>
  <string>Niki</string>
  <key>CFBundleExecutable</key>
  <string>Niki</string>
  <key>CFBundleIconFile</key>
  <string>icon.icns</string>
  <key>CFBundleIdentifier</key>
  <string>app.niki.journal</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>Niki</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>{VERSION}</string>
  <key>CFBundleVersion</key>
  <string>{VERSION}</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
"#
    );
    fs::write(contents_dir.join("Info.plist"), info_plist)
        .map_err(|e| format!("Info.plist 생성 실패: {e}"))?;

    let _ = Command::new("/usr/bin/xattr")
        .args(["-cr"])
        .arg(bundle_path)
        .status();
    let _ = Command::new("/usr/bin/codesign")
        .args(["--force", "--deep", "--sign", "-"])
        .arg(bundle_path)
        .status();

    Ok(bundle_path.to_path_buf())
}

#[cfg(target_os = "windows")]
pub fn run_installer_ui(silent: bool) -> Result<PathBuf, String> {
    let target_dir = install_dir()?;
    let target_exe = target_dir.join("Niki.exe");
    let target_ico = target_dir.join("icon.ico");
    let current_exe = env::current_exe().map_err(|e| e.to_string())?;

    if !silent {
        let prompt = format!(
            "Niki v{VERSION} 프로그램을 이 PC에 설치하시겠습니까?\n\n• 설치 위치: {}\n• 바탕화면 및 시작 메뉴 바로가기 생성\n• Windows [설치된 앱] 등록",
            target_dir.display()
        );
        if !confirm_dialog("Niki 설치 마법사", &prompt) {
            return Err("설치가 취소되었습니다.".into());
        }
    }

    stop_other_instances();
    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("설치 폴더 생성 실패: {e}"))?;

    if normalize_path(&current_exe) != normalize_path(&target_exe) {
        let mut last_err = None;
        for _ in 0..8 {
            match fs::copy(&current_exe, &target_exe) {
                Ok(_) => {
                    last_err = None;
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
        }
        if let Some(e) = last_err {
            return Err(format!("실행 파일 복사 실패: {e}"));
        }
    }

    let _ = fs::write(&target_ico, APP_ICON_ICO);

    let exe_size_kb = fs::metadata(&target_exe)
        .map(|m| (m.len() / 1024) as u32)
        .unwrap_or(12000);

    register_windows_shortcuts_and_uninstall(&target_dir, &target_exe, &target_ico, exe_size_kb)?;

    Command::new(&target_exe)
        .spawn()
        .map_err(|e| format!("설치된 Niki 실행 실패: {e}"))?;

    Ok(target_exe)
}

#[cfg(target_os = "macos")]
pub fn run_installer_ui(silent: bool) -> Result<PathBuf, String> {
    let target_bundle = install_dir()?;
    let current_exe = env::current_exe().map_err(|e| e.to_string())?;

    if !silent {
        let prompt = format!(
            "Niki v{VERSION} 앱을 이 Mac에 설치하시겠습니까?\n\n• 설치 위치: {}",
            target_bundle.display()
        );
        if !confirm_dialog("Niki 설치", &prompt) {
            return Err("설치가 취소되었습니다.".into());
        }
    }

    stop_other_instances();
    if let Some(parent) = target_bundle.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Applications 폴더 준비 실패: {e}"))?;
    }

    create_macos_app_bundle(&current_exe, &target_bundle)?;

    Command::new("/usr/bin/open")
        .arg("-a")
        .arg(&target_bundle)
        .spawn()
        .map_err(|e| format!("설치된 Niki.app 실행 실패: {e}"))?;

    Ok(target_bundle)
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn run_installer_ui(_silent: bool) -> Result<PathBuf, String> {
    Err("이 운영체제에서는 자동 설치가 지원되지 않습니다.".into())
}

#[cfg(target_os = "windows")]
pub fn run_uninstaller_ui() -> Result<(), String> {
    if !confirm_dialog(
        "Niki 제거",
        "이 PC에서 Niki 프로그램을 제거하시겠습니까?\n(작성하신 일기 데이터와 설정은 삭제되지 않고 안전하게 보존됩니다.)",
    ) {
        return Ok(());
    }

    stop_other_instances();

    let target_dir = install_dir()?;
    let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
$desktop = [Environment]::GetFolderPath('Desktop')
$startMenu = [Environment]::GetFolderPath('Programs')
Remove-Item -LiteralPath (Join-Path $desktop 'Niki.lnk') -Force
Remove-Item -LiteralPath (Join-Path $startMenu 'Niki.lnk') -Force
Remove-Item -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\app.niki.journal' -Recurse -Force
"#;
    let _ = run_powershell(script, &[]);

    info_dialog("Niki 제거 완료", "Niki 프로그램이 PC에서 제거되었습니다.");

    let dir_str = target_dir.to_string_lossy().to_string();
    let _ = Command::new("cmd.exe")
        .args([
            "/C",
            &format!("timeout /t 2 /nobreak >nul & rmdir /s /q \"{dir_str}\""),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn run_uninstaller_ui() -> Result<(), String> {
    if !confirm_dialog(
        "Niki 제거",
        "이 Mac에서 Niki.app을 제거하시겠습니까?\n(작성하신 일기 데이터와 설정은 삭제되지 않고 안전하게 보존됩니다.)",
    ) {
        return Ok(());
    }

    stop_other_instances();
    let target_bundle = install_dir()?;
    if target_bundle.exists() {
        let _ = fs::remove_dir_all(&target_bundle);
    }
    info_dialog("Niki 제거 완료", "Niki.app이 Applications 폴더에서 제거되었습니다.");
    Ok(())
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn run_uninstaller_ui() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn stop_other_instances() {
    let my_pid = std::process::id().to_string();
    let script = r#"
$myPid = [int]$env:NIKI_SELF_PID
Get-Process -Name 'Niki', 'niki_journal' -ErrorAction SilentlyContinue |
  Where-Object { $_.Id -ne $myPid } |
  Stop-Process -Force -ErrorAction SilentlyContinue
"#;
    let _ = run_powershell(script, &[("NIKI_SELF_PID", &my_pid)]);
    std::thread::sleep(Duration::from_millis(250));
}

#[cfg(target_os = "macos")]
fn stop_other_instances() {
    let my_pid = std::process::id().to_string();
    let script = format!(
        "for pid in $(pgrep -x Niki 2>/dev/null; pgrep -x niki_journal 2>/dev/null); do [ \"$pid\" != \"{my_pid}\" ] && kill -TERM \"$pid\" 2>/dev/null; done; exit 0"
    );
    let _ = Command::new("/bin/sh").args(["-c", &script]).status();
    std::thread::sleep(Duration::from_millis(250));
}

#[cfg(target_os = "windows")]
fn register_windows_shortcuts_and_uninstall(
    target_dir: &Path,
    target_exe: &Path,
    target_ico: &Path,
    size_kb: u32,
) -> Result<(), String> {
    let dir_s = target_dir.to_string_lossy().to_string();
    let exe_s = target_exe.to_string_lossy().to_string();
    let ico_s = target_ico.to_string_lossy().to_string();
    let size_s = size_kb.to_string();
    let uninstall_cmd = format!("\"{exe_s}\" --uninstall");

    let script = r#"
$ErrorActionPreference = 'Stop'
$wsh = New-Object -ComObject WScript.Shell
$desktop = [Environment]::GetFolderPath('Desktop')
$startMenu = [Environment]::GetFolderPath('Programs')

foreach ($folder in @($desktop, $startMenu)) {
  if ($folder -and (Test-Path -LiteralPath $folder)) {
    $lnk = $wsh.CreateShortcut((Join-Path $folder 'Niki.lnk'))
    $lnk.TargetPath = $env:NIKI_EXE
    $lnk.WorkingDirectory = $env:NIKI_DIR
    $lnk.IconLocation = "$($env:NIKI_ICO),0"
    $lnk.Description = 'Niki - 나만의 일기장'
    $lnk.Save()
  }
}

$regPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\app.niki.journal'
if (-not (Test-Path -Path $regPath)) {
  New-Item -Path $regPath -Force | Out-Null
}
Set-ItemProperty -Path $regPath -Name 'DisplayName' -Value 'Niki' -Type String
Set-ItemProperty -Path $regPath -Name 'DisplayVersion' -Value $env:NIKI_VER -Type String
Set-ItemProperty -Path $regPath -Name 'Publisher' -Value 'Niki' -Type String
Set-ItemProperty -Path $regPath -Name 'InstallLocation' -Value $env:NIKI_DIR -Type String
Set-ItemProperty -Path $regPath -Name 'DisplayIcon' -Value "$($env:NIKI_ICO),0" -Type String
Set-ItemProperty -Path $regPath -Name 'UninstallString' -Value $env:NIKI_UNINSTALL -Type String
Set-ItemProperty -Path $regPath -Name 'NoModify' -Value 1 -Type DWord
Set-ItemProperty -Path $regPath -Name 'NoRepair' -Value 1 -Type DWord
Set-ItemProperty -Path $regPath -Name 'EstimatedSize' -Value ([int]$env:NIKI_SIZE_KB) -Type DWord
"#;

    let _ = UNINSTALL_REG_KEY;
    run_powershell(
        script,
        &[
            ("NIKI_DIR", &dir_s),
            ("NIKI_EXE", &exe_s),
            ("NIKI_ICO", &ico_s),
            ("NIKI_VER", VERSION),
            ("NIKI_UNINSTALL", &uninstall_cmd),
            ("NIKI_SIZE_KB", &size_s),
        ],
    )
}

#[cfg(target_os = "windows")]
fn confirm_dialog(title: &str, message: &str) -> bool {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$res = [System.Windows.Forms.MessageBox]::Show(
  $env:NIKI_MSG,
  $env:NIKI_TITLE,
  [System.Windows.Forms.MessageBoxButtons]::YesNo,
  [System.Windows.Forms.MessageBoxIcon]::Question
)
if ($res -eq [System.Windows.Forms.DialogResult]::Yes) { exit 0 } else { exit 1 }
"#;
    run_powershell(script, &[("NIKI_TITLE", title), ("NIKI_MSG", message)]).is_ok()
}

#[cfg(target_os = "macos")]
fn confirm_dialog(title: &str, message: &str) -> bool {
    let script = "on run argv\ndisplay dialog (item 2 of argv) with title (item 1 of argv) buttons {\"취소\", \"확인\"} default button \"확인\"\nend run";
    Command::new("/usr/bin/osascript")
        .args(["-e", script, title, message])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn confirm_dialog(_title: &str, _message: &str) -> bool {
    true
}

#[cfg(target_os = "windows")]
fn info_dialog(title: &str, message: &str) {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.MessageBox]::Show(
  $env:NIKI_MSG,
  $env:NIKI_TITLE,
  [System.Windows.Forms.MessageBoxButtons]::OK,
  [System.Windows.Forms.MessageBoxIcon]::Information
) | Out-Null
"#;
    let _ = run_powershell(script, &[("NIKI_TITLE", title), ("NIKI_MSG", message)]);
}

#[cfg(target_os = "macos")]
fn info_dialog(title: &str, message: &str) {
    let script = "on run argv\ndisplay dialog (item 2 of argv) with title (item 1 of argv) buttons {\"확인\"} default button \"확인\"\nend run";
    let _ = Command::new("/usr/bin/osascript")
        .args(["-e", script, title, message])
        .status();
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn info_dialog(_title: &str, _message: &str) {}

#[cfg(target_os = "windows")]
pub fn error_dialog(title: &str, message: &str) {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.MessageBox]::Show(
  $env:NIKI_MSG,
  $env:NIKI_TITLE,
  [System.Windows.Forms.MessageBoxButtons]::OK,
  [System.Windows.Forms.MessageBoxIcon]::Error
) | Out-Null
"#;
    let _ = run_powershell(script, &[("NIKI_TITLE", title), ("NIKI_MSG", message)]);
}

#[cfg(target_os = "macos")]
pub fn error_dialog(title: &str, message: &str) {
    let script = "on run argv\ndisplay dialog (item 2 of argv) with title (item 1 of argv) buttons {\"확인\"} default button \"확인\" with icon stop\nend run";
    let _ = Command::new("/usr/bin/osascript")
        .args(["-e", script, title, message])
        .status();
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn error_dialog(title: &str, message: &str) {
    eprintln!("{title}: {message}");
}

#[cfg(target_os = "windows")]
fn run_powershell(script: &str, env_vars: &[(&str, &str)]) -> Result<(), String> {
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW);
    for (k, v) in env_vars {
        cmd.env(k, v);
    }
    let status = cmd
        .status()
        .map_err(|e| format!("PowerShell 실행 실패: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("설치 스크립트 실행이 중단되었습니다.".into())
    }
}
