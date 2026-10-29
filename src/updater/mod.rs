use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use reqwest::blocking::Client;
use ring::{
    digest::{SHA256, digest},
    rand::SystemRandom,
    signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey},
};
use serde::{Deserialize, Serialize};
use std::{ collections::HashMap, env, fs, path::Path, process::Command, time::Duration };

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdaterConfig {
    pub endpoint: String,
    pub pubkey: String,
    pub auto_check: bool,
    pub skipped_version: String,
}

impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            pubkey: String::new(),
            auto_check: true,
            skipped_version: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub notes: String,
    pub pub_date: String,
    pub download_url: String,
    pub signature: String,
    pub sha256: String,
    pub endpoint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SigningKeyPairInfo {
    pub public_key_base64: String,
    pub private_key_pkcs8_base64: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
struct LatestManifest {
    version: String,
    notes: String,
    pub_date: String,
    url: String,
    signature: String,
    sha256: String,
    platforms: HashMap<String, PlatformEntry>,
}

impl Default for LatestManifest {
    fn default() -> Self {
        Self {
            version: String::new(),
            notes: String::new(),
            pub_date: String::new(),
            url: String::new(),
            signature: String::new(),
            sha256: String::new(),
            platforms: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct PlatformEntry {
    url: String,
    signature: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    body: Option<String>,
    published_at: Option<String>,
    assets: Vec<GithubAsset>,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub fn cleanup_old_executable() {
    if let Ok(exe) = env::current_exe() {
        let old = exe.with_extension("exe.old");
        if old.exists() {
            let _ = fs::remove_file(&old);
        }
        let new_tmp = exe.with_extension("exe.new");
        if new_tmp.exists() {
            let _ = fs::remove_file(&new_tmp);
        }
    }
}

pub fn effective_config(saved: &UpdaterConfig) -> UpdaterConfig {
    let mut cfg = saved.clone();
    if cfg.endpoint.trim().is_empty() || cfg.endpoint.contains("YOUR_GITHUB_ID") {
        if let Some(built_in) = option_env!("NIKI_UPDATE_ENDPOINT")
            && !built_in.contains("YOUR_GITHUB_ID")
        {
            cfg.endpoint = built_in.trim().to_string();
        }
    }
    if cfg.pubkey.trim().is_empty() {
        if let Some(built_in_key) = option_env!("NIKI_UPDATE_PUBKEY") {
            cfg.pubkey = built_in_key.trim().to_string();
        }
    }
    if (cfg.endpoint.trim().is_empty() || cfg.endpoint.contains("YOUR_GITHUB_ID"))
        && let Some((ep, pk)) = read_runtime_updater_config()
    {
        if !ep.is_empty() {
            cfg.endpoint = ep;
        }
        if cfg.pubkey.trim().is_empty() && !pk.is_empty() {
            cfg.pubkey = pk;
        }
    }
    cfg
}

fn read_runtime_updater_config() -> Option<(String, String)> {
    let mut candidates = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("updater-config.json"));
    }
    if let Ok(exe) = env::current_exe()
        && let Some(parent) = exe.parent()
    {
        candidates.push(parent.join("updater-config.json"));
    }
    for path in candidates {
        if let Ok(raw) = fs::read_to_string(&path)
            && let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw)
        {
            let ep = val
                .get("endpoint")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let pk = val
                .get("pubkey")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if !ep.is_empty() && !ep.contains("YOUR_GITHUB_ID") {
                return Some((ep, pk));
            }
        }
    }
    None
}

pub fn normalize_endpoint(input: &str) -> Result<String, String> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err("업데이트 서버 주소(Endpoint URL)가 설정되지 않았습니다.".into());
    }
    if raw.contains("YOUR_GITHUB_ID") {
        return Err("updater-config.json 또는 설정에서 실제 GitHub 저장소나 웹 서버 주소를 입력해 주세요.".into());
    }
    // Shorthand "owner/repo" -> GitHub Releases latest.json URL
    if !raw.starts_with("http://") && !raw.starts_with("https://") {
        let parts: Vec<&str> = raw.trim_matches('/').split('/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Ok(format!(
                "https://github.com/{}/{}/releases/latest/download/latest.json",
                parts[0], parts[1]
            ));
        }
        return Err("업데이트 주소는 https://... 형식 또는 owner/repo 형식이어야 합니다.".into());
    }
    // If user pasted "https://github.com/owner/repo" or "https://github.com/owner/repo/releases", normalize to latest.json
    if let Some(rest) = raw.strip_prefix("https://github.com/") {
        let trimmed = rest.trim_end_matches('/');
        let segs: Vec<&str> = trimmed.split('/').collect();
        if segs.len() == 2 || (segs.len() == 3 && segs[2] == "releases") {
            return Ok(format!(
                "https://github.com/{}/{}/releases/latest/download/latest.json",
                segs[0], segs[1]
            ));
        }
    }
    Ok(raw.to_string())
}

fn github_api_fallback_url(endpoint: &str) -> Option<String> {
    let rest = endpoint.strip_prefix("https://github.com/")?;
    let segs: Vec<&str> = rest.split('/').collect();
    if segs.len() >= 2 {
        Some(format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            segs[0], segs[1]
        ))
    } else {
        None
    }
}

fn http_client(timeout_secs: u64) -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(format!("Niki-Updater/{CURRENT_VERSION}"))
        .build()
        .map_err(|e| format!("네트워크 클라이언트 초기화 실패: {e}"))
}

pub fn is_newer_version(current: &str, candidate: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.trim()
            .trim_start_matches(['v', 'V'])
            .split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let cur = parse(current);
    let cand = parse(candidate);
    let len = cur.len().max(cand.len());
    for i in 0..len {
        let a = *cur.get(i).unwrap_or(&0);
        let b = *cand.get(i).unwrap_or(&0);
        if b > a {
            return true;
        }
        if b < a {
            return false;
        }
    }
    false
}

pub fn check_update(saved: &UpdaterConfig) -> Result<UpdateInfo, String> {
    let cfg = effective_config(saved);
    let endpoint = normalize_endpoint(&cfg.endpoint)?;
    let client = http_client(12)?;

    let resp = client.get(&endpoint).send();
    match resp {
        Ok(r) if r.status().is_success() => {
            let text = r.text().map_err(|e| format!("응답 읽기 실패: {e}"))?;
            parse_manifest_or_release(&text, &endpoint)
        }
        Ok(r) => {
            if let Some(api_url) = github_api_fallback_url(&endpoint)
                && let Ok(api_resp) = client.get(&api_url).send()
                && api_resp.status().is_success()
                && let Ok(text) = api_resp.text()
            {
                return parse_manifest_or_release(&text, &endpoint);
            }
            Err(format!(
                "업데이트 정보를 가져오지 못했습니다 (HTTP {}). 릴리스에 latest.json 파일이 업로드되어 있는지 확인하세요.",
                r.status()
            ))
        }
        Err(e) => Err(format!("업데이트 서버 연결 실패: {e}")),
    }
}

fn parse_manifest_or_release(text: &str, endpoint: &str) -> Result<UpdateInfo, String> {
    if let Ok(manifest) = serde_json::from_str::<LatestManifest>(text)
        && !manifest.version.trim().is_empty()
    {
        let platform = manifest
            .platforms
            .get("windows-x86_64")
            .or_else(|| manifest.platforms.get("windows-x86_64-msvc"))
            .or_else(|| manifest.platforms.get("windows"))
            .cloned()
            .unwrap_or_default();

        let download_url = if !platform.url.is_empty() {
            platform.url
        } else {
            manifest.url
        };
        let signature = if !platform.signature.is_empty() {
            platform.signature
        } else {
            manifest.signature
        };
        let sha256 = if !platform.sha256.is_empty() {
            platform.sha256
        } else {
            manifest.sha256
        };
        let clean_ver = manifest.version.trim().trim_start_matches(['v', 'V']).to_string();
        let update_available = is_newer_version(CURRENT_VERSION, &clean_ver);
        return Ok(UpdateInfo {
            current_version: CURRENT_VERSION.to_string(),
            latest_version: clean_ver,
            update_available,
            notes: manifest.notes,
            pub_date: manifest.pub_date,
            download_url,
            signature,
            sha256,
            endpoint: endpoint.to_string(),
        });
    }

    if let Ok(release) = serde_json::from_str::<GithubRelease>(text) {
        let clean_ver = release
            .tag_name
            .trim()
            .trim_start_matches(['v', 'V'])
            .to_string();
        let asset = release
            .assets
            .iter()
            .find(|a| {
                let n = a.name.to_ascii_lowercase();
                n.ends_with(".exe") || n.ends_with(".msi")
            });
        let download_url = asset.map(|a| a.browser_download_url.clone()).unwrap_or_default();
        let update_available = is_newer_version(CURRENT_VERSION, &clean_ver) && !download_url.is_empty();
        return Ok(UpdateInfo {
            current_version: CURRENT_VERSION.to_string(),
            latest_version: clean_ver,
            update_available,
            notes: release.body.unwrap_or_default(),
            pub_date: release.published_at.unwrap_or_default(),
            download_url,
            signature: String::new(),
            sha256: String::new(),
            endpoint: endpoint.to_string(),
        });
    }

    Err("업데이트 매니페스트(latest.json) 형식을 해석할 수 없습니다.".into())
}

fn decode_b64_or_hex(s: &str) -> Result<Vec<u8>, String> {
    let trimmed = s.trim();
    if trimmed.len() % 2 == 0 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        let mut out = Vec::with_capacity(trimmed.len() / 2);
        for i in (0..trimmed.len()).step_by(2) {
            if let Ok(v) = u8::from_str_radix(&trimmed[i..i + 2], 16) {
                out.push(v);
            }
        }
        if out.len() == 32 || out.len() == 64 {
            return Ok(out);
        }
    }
    B64.decode(trimmed)
        .map_err(|_| "키 또는 서명(Base64/Hex) 형식이 올바르지 않습니다.".to_string())
}

pub fn verify_payload(bytes: &[u8], pubkey_b64: &str, signature_b64: &str, expected_sha256: &str) -> Result<(), String> {
    if !expected_sha256.trim().is_empty() {
        let actual = hex_encode(digest(&SHA256, bytes).as_ref());
        if !actual.eq_ignore_ascii_case(expected_sha256.trim()) {
            return Err("다운로드한 업데이트 파일의 SHA-256 해시가 일치하지 않습니다.".into());
        }
    }
    if !pubkey_b64.trim().is_empty() && !signature_b64.trim().is_empty() {
        let pub_bytes = decode_b64_or_hex(pubkey_b64)?;
        let sig_bytes = decode_b64_or_hex(signature_b64)?;
        let key = UnparsedPublicKey::new(&ED25519, &pub_bytes);
        key.verify(bytes, &sig_bytes)
            .map_err(|_| "업데이트 파일의 디지털 서명 검증에 실패했습니다 (위변조 의심).")?;
    }
    Ok(())
}

pub fn download_and_apply_update(saved: &UpdaterConfig) -> Result<String, String> {
    let cfg = effective_config(saved);
    let info = check_update(&cfg)?;
    if !info.update_available {
        return Err("현재 이미 최신 버전입니다.".into());
    }
    if info.download_url.trim().is_empty() {
        return Err("업데이트 다운로드 주소(url)가 비어 있습니다.".into());
    }
    if !info.download_url.starts_with("https://") && !info.download_url.starts_with("http://127.0.0.1") && !info.download_url.starts_with("http://localhost") {
        return Err("보안을 위해 업데이트 다운로드는 HTTPS 주소만 허용됩니다.".into());
    }

    let client = http_client(180)?;
    let resp = client
        .get(&info.download_url)
        .send()
        .map_err(|e| format!("업데이트 파일 다운로드 실패: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("업데이트 파일 다운로드 실패 (HTTP {})", resp.status()));
    }
    let bytes = resp
        .bytes()
        .map_err(|e| format!("업데이트 데이터 수신 실패: {e}"))?;

    verify_payload(&bytes, &cfg.pubkey, &info.signature, &info.sha256)?;

    let lower_url = info.download_url.to_ascii_lowercase();
    let is_msi = lower_url.ends_with(".msi");
    let is_pe = bytes.len() > 64 && &bytes[0..2] == b"MZ";
    let is_msi_magic = bytes.len() > 8 && bytes[0..4] == [0xD0, 0xCF, 0x11, 0xE0];
    if !is_pe && !is_msi_magic {
        return Err("다운로드된 파일이 유효한 Windows 실행/설치 파일이 아닙니다.".into());
    }

    let is_installer = is_msi || lower_url.contains("setup") || lower_url.contains("install");
    if is_installer {
        let ext = if is_msi { "msi" } else { "exe" };
        let temp_path = env::temp_dir().join(format!("niki-update-{}.{ext}", info.latest_version));
        fs::write(&temp_path, &bytes)
            .map_err(|e| format!("임시 설치 파일 저장 실패: {e}"))?;
        if is_msi {
            Command::new("msiexec")
                .arg("/i")
                .arg(&temp_path)
                .spawn()
                .map_err(|e| format!("MSI 설치 프로그램 실행 실패: {e}"))?;
        } else {
            Command::new(&temp_path)
                .arg("--silent")
                .spawn()
                .map_err(|e| format!("설치 프로그램 실행 실패: {e}"))?;
        }
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(600));
            std::process::exit(0);
        });
        return Ok(info.latest_version);
    }

    // Portable / direct executable self-replacement
    let current_exe = env::current_exe()
        .map_err(|e| format!("현재 실행 파일 경로를 확인할 수 없습니다: {e}"))?;
    let new_exe = current_exe.with_extension("exe.new");
    let old_exe = current_exe.with_extension("exe.old");

    fs::write(&new_exe, &bytes)
        .map_err(|e| format!("새 실행 파일 기록 실패: {e}"))?;
    if old_exe.exists() {
        let _ = fs::remove_file(&old_exe);
    }
    fs::rename(&current_exe, &old_exe)
        .map_err(|e| format!("기존 실행 파일 이름 변경 실패: {e}"))?;
    if let Err(e) = fs::rename(&new_exe, &current_exe) {
        let _ = fs::rename(&old_exe, &current_exe);
        return Err(format!("새 실행 파일 교체 실패: {e}"));
    }

    Command::new(&current_exe)
        .spawn()
        .map_err(|e| format!("업데이트된 앱 재시작 실패: {e}"))?;

    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(450));
        std::process::exit(0);
    });

    Ok(info.latest_version)
}

pub fn generate_signing_keypair() -> Result<SigningKeyPairInfo, String> {
    let rng = SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|_| "Ed25519 서명 키 쌍 생성에 실패했습니다.")?;
    let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
        .map_err(|_| "생성된 Ed25519 키 쌍 로드에 실패했습니다.")?;
    Ok(SigningKeyPairInfo {
        public_key_base64: B64.encode(pair.public_key().as_ref()),
        private_key_pkcs8_base64: B64.encode(pkcs8.as_ref()),
    })
}

pub fn sign_file_and_build_manifest(
    exe_path: &Path,
    version: &str,
    download_url: &str,
    notes: &str,
    private_key_b64: &str,
) -> Result<String, String> {
    let bytes = fs::read(exe_path)
        .map_err(|e| format!("서명할 파일을 읽을 수 없습니다: {e}"))?;
    let sha256 = hex_encode(digest(&SHA256, &bytes).as_ref());
    let signature = if !private_key_b64.trim().is_empty() {
        let pkcs8 = B64
            .decode(private_key_b64.trim())
            .map_err(|_| "개인키(Base64) 형식이 올바르지 않습니다.")?;
        let pair = Ed25519KeyPair::from_pkcs8(&pkcs8)
            .map_err(|_| "Ed25519 개인키 로드 실패")?;
        B64.encode(pair.sign(&bytes).as_ref())
    } else {
        String::new()
    };

    let clean_ver = version.trim().trim_start_matches(['v', 'V']);
    let pub_date = chrono::Utc::now().to_rfc3339();
    let manifest = serde_json::json!({
        "version": clean_ver,
        "notes": notes,
        "pub_date": pub_date,
        "platforms": {
            "windows-x86_64": {
                "url": download_url.trim(),
                "signature": signature,
                "sha256": sha256
            }
        }
    });
    serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_accurately() {
        assert!(is_newer_version("0.1.0", "0.1.1"));
        assert!(is_newer_version("0.1.0", "v0.2.0"));
        assert!(is_newer_version("0.1.9", "0.1.10"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.2.0", "0.1.9"));
    }

    #[test]
    fn normalizes_github_and_custom_endpoints() {
        assert_eq!(
            normalize_endpoint("myuser/niki").unwrap(),
            "https://github.com/myuser/niki/releases/latest/download/latest.json"
        );
        assert_eq!(
            normalize_endpoint("https://github.com/myuser/niki").unwrap(),
            "https://github.com/myuser/niki/releases/latest/download/latest.json"
        );
        assert_eq!(
            normalize_endpoint("https://updates.example.com/niki/latest.json").unwrap(),
            "https://updates.example.com/niki/latest.json"
        );
    }

    #[test]
    fn signs_and_verifies_payload_with_ed25519() {
        let kp = generate_signing_keypair().unwrap();
        let data = b"MZ-sample-binary-payload";
        let pkcs8 = B64.decode(&kp.private_key_pkcs8_base64).unwrap();
        let pair = Ed25519KeyPair::from_pkcs8(&pkcs8).unwrap();
        let sig = B64.encode(pair.sign(data).as_ref());
        let sha = hex_encode(digest(&SHA256, data).as_ref());

        assert!(verify_payload(data, &kp.public_key_base64, &sig, &sha).is_ok());
        assert!(verify_payload(b"MZ-tampered", &kp.public_key_base64, &sig, &sha).is_err());
    }
}
