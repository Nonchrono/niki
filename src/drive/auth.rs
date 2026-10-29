//! Google desktop OAuth and credential storage for Drive sync.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde::Deserialize;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};
use url::Url;

const SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";
const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const ABOUT: &str = "https://www.googleapis.com/drive/v3/about?fields=user(displayName,emailAddress)";

fn random_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "난수 생성 실패")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()).as_ref())
}

fn callback(request: &str, expected_state: &str) -> Result<String, String> {
    let line = request.lines().next().ok_or("인증 응답 없음")?;
    let mut parts = line.split_whitespace();
    if parts.next() != Some("GET") {
        return Err("잘못된 인증 요청".into());
    }
    let target = parts.next().ok_or("인증 경로 없음")?;
    if !target.starts_with("/callback?") {
        return Err("잘못된 인증 경로".into());
    }
    let url = Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| "잘못된 인증 응답")?;
    let mut state = None;
    let mut code = None;
    let mut error = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "state" if state.is_none() => state = Some(v.into_owned()),
            "code" if code.is_none() => code = Some(v.into_owned()),
            "error" if error.is_none() => error = Some(v.into_owned()),
            "state" | "code" | "error" => return Err("중복된 인증 응답".into()),
            _ => {}
        }
    }
    if state.as_deref() != Some(expected_state) {
        return Err("인증 상태 불일치".into());
    }
    if error.is_some() {
        return Err("Google 인증이 거부됐습니다".into());
    }
    code.filter(|s| !s.is_empty())
        .ok_or("인증 코드 누락".into())
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

/// Opens the system browser. Must be called off the UI thread.
pub fn authorize(client_id: &str, client_secret: Option<&str>) -> Result<AuthTokens, String> {
    if client_id.is_empty() || client_id.len() > 512 {
        return Err("OAuth 클라이언트 ID를 확인하세요".into());
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let redirect = format!(
        "http://127.0.0.1:{}/callback",
        listener.local_addr().map_err(|e| e.to_string())?.port()
    );
    let state = random_token()?;
    let verifier = random_token()?;
    let mut auth = Url::parse(AUTH).map_err(|e| e.to_string())?;
    auth.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", &redirect)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("state", &state)
        .append_pair("code_challenge", &challenge(&verifier))
        .append_pair("code_challenge_method", "S256")
        .append_pair("access_type", "offline")
        .append_pair("prompt", "select_account consent");
    #[cfg(target_os = "windows")]
    std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", auth.as_str()])
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(auth.as_str())
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    std::process::Command::new("xdg-open")
        .arg(auth.as_str())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(180);
    let code = loop {
        if Instant::now() >= deadline {
            return Err("Google 인증 시간이 초과됐습니다".into());
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .map_err(|e| e.to_string())?;
                let mut buf = [0u8; 8192];
                let n = match stream.read(&mut buf) {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                let result = std::str::from_utf8(&buf[..n])
                    .map_err(|_| "잘못된 인증 응답".to_string())
                    .and_then(|s| callback(s, &state));
                let response = if result.is_ok() {
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><html lang=\"ko\"><head><meta charset=\"utf-8\"><title>Niki — Google 로그인 완료</title><style>body{margin:0;height:100vh;display:grid;place-items:center;background:#f5f7f3;font-family:'Noto Sans KR',system-ui,sans-serif;color:#233d31}.card{background:#fff;border:1px solid #dfe7dd;border-radius:20px;padding:36px 40px;text-align:center;box-shadow:0 14px 36px rgba(35,61,49,.09);max-width:400px}.mark{width:54px;height:54px;border-radius:16px;background:#315e42;color:#fff;display:grid;place-items:center;font-size:26px;margin:0 auto 16px}h2{margin:0 0 8px;font-size:20px}p{margin:0;color:#6d7f71;font-size:13px;line-height:1.6}</style></head><body><div class=\"card\"><div class=\"mark\">✓</div><h2>Google 로그인이 완료되었습니다</h2><p>데스크톱 앱 <strong>Niki</strong>로 돌아갑니다. 이 창은 닫으셔도 됩니다.</p></div><script>window.close();setTimeout(()=>window.close(),600);</script></body></html>"
                } else {
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><html lang=\"ko\"><head><meta charset=\"utf-8\"><title>Niki — 인증 취소됨</title><style>body{margin:0;height:100vh;display:grid;place-items:center;background:#f5f7f3;font-family:system-ui,sans-serif;color:#233d31}.card{background:#fff;border:1px solid #e8cbc8;border-radius:20px;padding:32px;text-align:center;max-width:380px}h2{margin:0 0 8px;color:#994e46;font-size:19px}p{margin:0;color:#6d7f71;font-size:13px}</style></head><body><div class=\"card\"><h2>Google 로그인이 취소되었거나 실패했습니다</h2><p>이 창을 닫고 Niki 앱에서 다시 시도해 주세요.</p></div></body></html>"
                };
                let _ = stream.write_all(response.as_bytes());
                match result {
                    Ok(code) => break code,
                    Err(error) if error == "Google 인증이 거부됐습니다" => {
                        return Err(error);
                    }
                    Err(_) => {}
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    let mut form = vec![
        ("client_id", client_id),
        ("code", code.as_str()),
        ("code_verifier", verifier.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("grant_type", "authorization_code"),
    ];
    if let Some(secret) = client_secret {
        form.push(("client_secret", secret));
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "HTTP 초기화 실패")?;
    let token: TokenResponse = client
        .post(TOKEN)
        .form(&form)
        .send()
        .map_err(|_| "Google 토큰 요청 실패")?
        .error_for_status()
        .map_err(|_| "Google 토큰 교환 실패")?
        .json()
        .map_err(|_| "Google 토큰 응답 오류")?;
    if token.access_token.is_empty() {
        return Err("Google 액세스 토큰이 비어 있습니다".into());
    }
    Ok(AuthTokens {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
    })
}

#[cfg(target_os = "windows")]
fn credential_set(service: &str, account: &str, secret: &str) -> Result<(), String> {
    keyring::Entry::new(service, account)
        .map_err(|e| e.to_string())?
        .set_password(secret)
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn credential_get(service: &str, account: &str) -> Result<String, String> {
    keyring::Entry::new(service, account)
        .map_err(|e| e.to_string())?
        .get_password()
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn credential_delete(service: &str, account: &str) -> Result<(), String> {
    keyring::Entry::new(service, account)
        .map_err(|e| e.to_string())?
        .delete_credential()
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn credential_set(service: &str, account: &str, secret: &str) -> Result<(), String> {
    let output = std::process::Command::new("/usr/bin/security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            service,
            "-a",
            account,
            "-w",
            secret,
        ])
        .output()
        .map_err(|e| format!("macOS 키체인 실행 실패: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err("macOS 키체인에 자격 증명을 저장하지 못했습니다".into())
    }
}

#[cfg(target_os = "macos")]
fn credential_get(service: &str, account: &str) -> Result<String, String> {
    let output = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", service, "-a", account, "-w"])
        .output()
        .map_err(|e| format!("macOS 키체인 실행 실패: {e}"))?;
    if output.status.success() {
        let value = String::from_utf8_lossy(&output.stdout)
            .trim_end_matches(['\r', '\n'])
            .to_string();
        if value.is_empty() {
            Err("macOS 키체인 항목이 비어 있습니다".into())
        } else {
            Ok(value)
        }
    } else {
        Err("macOS 키체인에서 자격 증명을 찾을 수 없습니다".into())
    }
}

#[cfg(target_os = "macos")]
fn credential_delete(service: &str, account: &str) -> Result<(), String> {
    let output = std::process::Command::new("/usr/bin/security")
        .args(["delete-generic-password", "-s", service, "-a", account])
        .output()
        .map_err(|e| format!("macOS 키체인 실행 실패: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err("macOS 키체인에서 자격 증명을 삭제하지 못했습니다".into())
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn credential_set(_service: &str, _account: &str, _secret: &str) -> Result<(), String> {
    Err("이 운영체제에서는 키체인 저장이 지원되지 않습니다".into())
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn credential_get(_service: &str, _account: &str) -> Result<String, String> {
    Err("이 운영체제에서는 키체인 조회가 지원되지 않습니다".into())
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn credential_delete(_service: &str, _account: &str) -> Result<(), String> {
    Err("이 운영체제에서는 키체인 삭제가 지원되지 않습니다".into())
}

pub fn save_refresh_token(client_id: &str, token: &str) -> Result<(), String> {
    credential_set("Niki Journal Google Drive", client_id, token)
}

pub fn load_refresh_token(client_id: &str) -> Result<String, String> {
    credential_get("Niki Journal Google Drive", client_id)
}

pub fn refresh_access_token(
    client_id: &str,
    client_secret: Option<&str>,
    refresh_token: &str,
) -> Result<String, String> {
    let mut form = vec![
        ("client_id", client_id),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];
    if let Some(secret) = client_secret {
        form.push(("client_secret", secret));
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "HTTP 초기화 실패")?;
    let token: TokenResponse = client
        .post(TOKEN)
        .form(&form)
        .send()
        .map_err(|_| "Google 토큰 갱신 요청 실패")?
        .error_for_status()
        .map_err(|_| "Google 인증을 다시 연결해야 합니다")?
        .json()
        .map_err(|_| "Google 토큰 응답 오류")?;
    Ok(token.access_token)
}

pub fn delete_refresh_token(client_id: &str) -> Result<(), String> {
    credential_delete("Niki Journal Google Drive", client_id)
}

pub fn save_client_secret(client_id: &str, client_secret: &str) -> Result<(), String> {
    credential_set("Niki Journal OAuth Client", client_id, client_secret)
}

pub fn load_client_secret(client_id: &str) -> Option<String> {
    credential_get("Niki Journal OAuth Client", client_id)
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::env::var("NIKI_GOOGLE_CLIENT_SECRET")
                .ok()
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            option_env!("NIKI_GOOGLE_CLIENT_SECRET")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

pub fn acquire_access_token(client_id: &str) -> Result<String, String> {
    let refresh = load_refresh_token(client_id)
        .map_err(|_| "Google 계정이 연결되어 있지 않습니다".to_string())?;
    let secret = load_client_secret(client_id);
    refresh_access_token(client_id, secret.as_deref(), &refresh)
}

#[derive(Deserialize)]
struct AboutResponse {
    user: Option<AboutUser>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AboutUser {
    display_name: Option<String>,
    email_address: Option<String>,
}

pub fn fetch_user_profile(access_token: &str) -> Option<(String, String)> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;
    let about: AboutResponse = client
        .get(ABOUT)
        .bearer_auth(access_token)
        .send()
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .ok()?;
    let user = about.user?;
    let name = user.display_name.unwrap_or_default().trim().to_string();
    let email = user.email_address.unwrap_or_default().trim().to_string();
    if name.is_empty() && email.is_empty() {
        None
    } else {
        Some((name, email))
    }
}

pub fn validate_client_id(client_id: &str) -> Result<(), String> {
    if client_id.is_empty()
        || client_id.len() > 256
        || !client_id.ends_with(".apps.googleusercontent.com")
        || !client_id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    {
        return Err("Google OAuth 클라이언트 ID를 확인하세요 (.apps.googleusercontent.com)".into());
    }
    Ok(())
}

pub fn parse_credentials_json(bytes: &[u8]) -> Result<(String, String), String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| "올바른 Google OAuth JSON 파일이 아닙니다".to_string())?;
    let section = value
        .get("installed")
        .or_else(|| value.get("web"))
        .unwrap_or(&value);
    let client_id = section
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    validate_client_id(&client_id)?;
    let client_secret = section
        .get("client_secret")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if client_secret.len() > 512 {
        return Err("클라이언트 보안 비밀(client_secret)이 너무 깁니다".into());
    }
    Ok((client_id, client_secret))
}

pub fn save_sync_key(key: &[u8; 32]) -> Result<(), String> {
    let encoded = crate::sync_crypto::encode_recovery_key(key);
    credential_set("Niki Journal Sync Key", "master", &encoded)
}

pub fn load_sync_key() -> Result<[u8; 32], String> {
    let encoded = credential_get("Niki Journal Sync Key", "master")
        .map_err(|_| "동기화 복구 키가 설정되지 않았습니다".to_string())?;
    crate::sync_crypto::decode_recovery_key(&encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_state_and_rejects_duplicate_parameters() {
        assert_eq!(
            callback("GET /callback?state=abc&code=hello HTTP/1.1\r\n", "abc").unwrap(),
            "hello"
        );
        assert!(callback("GET /callback?state=wrong&code=hello HTTP/1.1\r\n", "abc").is_err());
        assert!(
            callback(
                "GET /callback?state=abc&state=abc&code=hello HTTP/1.1\r\n",
                "abc"
            )
            .is_err()
        );
        assert!(callback("GET /other?state=abc&code=hello HTTP/1.1\r\n", "abc").is_err());
        assert_eq!(
            callback(
                "GET /callback?state=abc&error=access_denied HTTP/1.1\r\n",
                "abc"
            )
            .unwrap_err(),
            "Google 인증이 거부됐습니다"
        );
    }
    #[test]
    fn pkce_matches_rfc_example() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn parses_google_oauth_credentials_json() {
        let installed = br#"{"installed":{"client_id":"12345-abc.apps.googleusercontent.com","client_secret":"GOCSPX-secret123"}}"#;
        assert_eq!(
            parse_credentials_json(installed).unwrap(),
            (
                "12345-abc.apps.googleusercontent.com".into(),
                "GOCSPX-secret123".into()
            )
        );
        let direct = br#"{"client_id":"999-xyz.apps.googleusercontent.com"}"#;
        assert_eq!(
            parse_credentials_json(direct).unwrap(),
            ("999-xyz.apps.googleusercontent.com".into(), String::new())
        );
        assert!(parse_credentials_json(br#"{"installed":{"client_id":"invalid"}}"#).is_err());
    }
}
