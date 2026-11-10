use super::{
    storage::{Account, Credentials},
    *,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpListener,
    time::Instant,
};
use url::Url;

const AUTHORIZE: &str = "https://auth.openai.com/api/accounts/authorize";
const JWKS: &str = "https://auth.openai.com/.well-known/jwks.json";
const REVOKE: &str = "https://auth.openai.com/api/accounts/oauth/revoke";
const DYNAMIC: &str = "dynamic_agent_client";

fn random() -> Result<String, String> {
    let mut bytes = [0; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "인증 난수 생성 실패")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()))
}

pub(super) fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("rundll32.exe");
        c.creation_flags(0x08000000)
            .arg("url.dll,FileProtocolHandler")
            .arg(url);
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("/usr/bin/open");
        c.arg(url);
        c
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| "브라우저를 열지 못했습니다")?;
    Ok(())
}

fn authorize_url(
    host: &str,
    previous: Option<&Account>,
    redirect: &str,
    state: &str,
    nonce: &str,
    verifier: &str,
) -> Url {
    let mut url = Url::parse(AUTHORIZE).unwrap();
    let mut query = url.query_pairs_mut();
    query
        .append_pair(
            "client_id",
            previous.map_or(DYNAMIC, |a| a.client_id.as_str()),
        )
        .append_pair("ext_agent_host_id", host)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect)
        .append_pair("scope", SCOPES)
        .append_pair("resource", RESOURCE)
        .append_pair("state", state)
        .append_pair("nonce", nonce)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &challenge(verifier));
    if let Some(account) = previous {
        if !account.email.is_empty() {
            query.append_pair("login_hint", &account.email);
        }
        if let Some(credentials) = &account.credentials {
            query.append_pair("id_token_hint", &credentials.id_token);
            if !has_plan(&credentials.scopes) {
                query.append_pair("prompt", "consent");
            }
        }
    } else {
        query.append_pair("agent_name_hint", "Niki");
    }
    drop(query);
    url
}

fn parse_callback(
    request: &str,
    state: &str,
    previous: Option<&Account>,
) -> Result<(String, String), String> {
    let mut parts = request
        .lines()
        .next()
        .ok_or("잘못된 인증 응답")?
        .split_whitespace();
    if parts.next() != Some("GET") {
        return Err("잘못된 인증 요청".into());
    }
    let target = parts.next().ok_or("인증 경로 누락")?;
    if !target.starts_with("/auth/callback?") {
        return Err("잘못된 인증 경로".into());
    }
    let url = Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| "잘못된 인증 응답")?;
    let mut values = HashMap::new();
    for (key, value) in url.query_pairs() {
        if values
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return Err("중복된 인증 응답".into());
        }
    }
    if values.get("state").map(String::as_str) != Some(state) {
        return Err("인증 상태가 일치하지 않습니다".into());
    }
    if values.contains_key("error") {
        return Err("ChatGPT 연결이 승인되지 않았습니다. 다시 연결하거나 취소해 주세요.".into());
    }
    let code = values
        .get("code")
        .filter(|s| !s.is_empty())
        .ok_or("인증 코드 누락")?;
    let client_id = match (previous, values.get("client_id")) {
        (Some(a), None) => a.client_id.clone(),
        (Some(a), Some(id)) if id == &a.client_id => id.clone(),
        (Some(_), _) => return Err("연결하려던 계정의 클라이언트 ID와 일치하지 않습니다".into()),
        (None, Some(id)) if id.starts_with("oaiapp_") && id.len() < 256 => id.clone(),
        _ => return Err("ChatGPT 앱 등록이 완료되지 않았습니다".into()),
    };
    Ok((code.clone(), client_id))
}

#[derive(Clone, Deserialize)]
struct Claims {
    sub: String,
    email: Option<String>,
    nonce: Option<String>,
    azp: Option<String>,
    aud: Value,
    iat: i64,
}

fn validate_identity(
    token: &str,
    jwks: &JwkSet,
    client_id: &str,
    nonce: Option<&str>,
) -> Result<Claims, String> {
    let header = decode_header(token).map_err(|_| "ChatGPT ID 토큰 형식 오류")?;
    if header.alg != Algorithm::RS256 {
        return Err("지원하지 않는 ID 토큰 서명 방식".into());
    }
    let jwk = jwks
        .find(header.kid.as_deref().ok_or("ID 토큰 서명 키 누락")?)
        .ok_or("ID 토큰 서명 키를 찾지 못했습니다")?;
    let key = DecodingKey::from_jwk(jwk).map_err(|_| "ID 토큰 서명 키 오류")?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[client_id]);
    validation.set_required_spec_claims(&["sub", "exp", "iat", "iss", "aud"]);
    validation.leeway = 5;
    validation.validate_nbf = true;
    let claims = decode::<Claims>(token, &key, &validation)
        .map_err(|_| "ChatGPT ID 토큰 서명 또는 유효기간 검증에 실패했습니다")?
        .claims;
    if claims.sub.is_empty()
        || claims.iat > chrono::Utc::now().timestamp() + 5
        || nonce.is_some_and(|n| claims.nonce.as_deref() != Some(n))
        || claims.azp.as_deref().is_some_and(|a| a != client_id)
        || (claims.aud.as_array().is_some_and(|a| a.len() > 1)
            && claims.azp.as_deref() != Some(client_id))
    {
        return Err("ChatGPT ID 토큰의 계정 또는 로그인 요청 정보가 일치하지 않습니다".into());
    }
    Ok(claims)
}

fn identity(
    http: &reqwest::blocking::Client,
    token: &str,
    client_id: &str,
    nonce: Option<&str>,
) -> Result<Claims, String> {
    // One fetch per login/renewal that supplies an ID token; never trust a token-supplied key URL.
    let response = http
        .get(JWKS)
        .send()
        .map_err(|_| "ChatGPT 인증 서명 키를 가져오지 못했습니다")?;
    let jwks: JwkSet = serde_json::from_value(json_response(response)?)
        .map_err(|_| "ChatGPT 서명 키 응답 오류")?;
    validate_identity(token, &jwks, client_id, nonce)
}

pub(super) fn json_response(response: reqwest::blocking::Response) -> Result<Value, String> {
    let (status, value, request_id) = response_parts(response)?;
    if !(200..300).contains(&status) {
        return Err(api_error(status, &value, &request_id));
    }
    if value.is_null() {
        return Err("ChatGPT가 올바르지 않은 응답을 반환했습니다".into());
    }
    Ok(value)
}

fn response_parts(response: reqwest::blocking::Response) -> Result<(u16, Value, String), String> {
    let status = response.status().as_u16();
    let request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let mut bytes = Vec::new();
    response
        .take(2 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|_| "ChatGPT 응답을 읽지 못했습니다")?;
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Ok((status, value, request_id))
}

pub(super) fn api_error(status: u16, value: &Value, request_id: &str) -> String {
    let code = value
        .pointer("/error/code")
        .and_then(Value::as_str)
        .or_else(|| value.get("error").and_then(Value::as_str))
        .unwrap_or("");
    let message = match code {
        "subscription_sharing_usage_limit_exceeded" => {
            "ChatGPT 또는 Niki의 사용량 한도에 도달했습니다. 사용량 관리에서 확인해 주세요."
        }
        "subscription_sharing_user_not_eligible" => {
            "이 ChatGPT 계정 또는 워크스페이스는 구독 연결을 사용할 수 없습니다."
        }
        "subscription_sharing_usage_unavailable" | "subscription_sharing_user_unavailable" => {
            "ChatGPT 사용 가능 여부를 확인하지 못했습니다. 잠시 후 다시 시도하세요."
        }
        "subscription_sharing_unsupported_capability" => {
            "현재 계정에서 이 요약 요청을 지원하지 않습니다."
        }
        "invalid_grant"
        | "invalid_refresh_token"
        | "token_expired"
        | "refresh_token_expired"
        | "refresh_token_invalidated"
        | "refresh_token_reused" => {
            "ChatGPT 인증이 만료되거나 해제됐습니다. 설정에서 다시 연결해 주세요."
        }
        _ if status == 401 => {
            "ChatGPT 인증 또는 구독 사용 권한을 확인하지 못했습니다. 설정에서 연결 상태를 확인하세요."
        }
        _ if status == 403 => "ChatGPT 계정의 권한 또는 제공 지역에서 이 요청을 허용하지 않습니다.",
        _ if status == 429 => {
            "ChatGPT 요청 한도에 도달했습니다. 사용량 관리에서 확인하고 잠시 후 다시 시도하세요."
        }
        _ if status >= 500 => "ChatGPT 서비스에 일시적인 문제가 있습니다. 잠시 후 다시 시도하세요.",
        _ => "ChatGPT 요청을 완료하지 못했습니다. 연결 상태를 확인해 주세요.",
    };
    let safe = |s: &str| {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .take(100)
            .collect::<String>()
    };
    let kind = if status == 0 {
        "응답 오류".into()
    } else {
        format!("HTTP {status}")
    };
    format!(
        "{message} ({kind}{}{})",
        if code.is_empty() {
            String::new()
        } else {
            format!(", {}", safe(code))
        },
        if request_id.is_empty() {
            String::new()
        } else {
            format!(", 요청 {}", safe(request_id))
        }
    )
}

fn credentials(value: &Value, old: Option<&Credentials>) -> Result<Credentials, String> {
    let field = |key| {
        value
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    if field("token_type").is_none_or(|s| !s.eq_ignore_ascii_case("Bearer")) {
        return Err("ChatGPT 토큰 유형 오류".into());
    }
    let expires = value
        .get("expires_in")
        .and_then(Value::as_i64)
        .filter(|s| *s > 0 && *s <= 86400)
        .ok_or("ChatGPT 토큰 유효기간 오류")?;
    Ok(Credentials {
        access_token: field("access_token").ok_or("ChatGPT 액세스 토큰 누락")?,
        refresh_token: field("refresh_token").ok_or("ChatGPT 갱신 토큰 누락")?,
        id_token: field("id_token")
            .or_else(|| old.map(|c| c.id_token.clone()))
            .ok_or("ChatGPT ID 토큰 누락")?,
        scopes: field("scope")
            .or_else(|| old.map(|c| c.scopes.clone()))
            .ok_or("ChatGPT 사용 권한 응답 누락")?,
        expires_at: chrono::Utc::now().timestamp() + expires,
    })
}

pub(super) fn connect(
    http: &reqwest::blocking::Client,
    host: &str,
    previous: Option<&Account>,
    pending_client_id: Option<&str>,
    operation: &Operation,
    mut remember: impl FnMut(&str) -> Result<(), String>,
) -> Result<Account, String> {
    let mut registration = previous.cloned().or_else(|| {
        pending_client_id.map(|id| Account {
            client_id: id.into(),
            ..Default::default()
        })
    });
    for attempt in 0..2 {
        operation.check()?;
        match connect_once(http, host, registration.as_ref(), operation, &mut remember) {
            Ok(account) => return Ok(account),
            Err(error) => {
                if attempt == 0
                    && let Some(client_id) = error.retry_client
                {
                    // Registration can issue an unusable first code. Repeat authorization
                    // with fresh PKCE/state/nonce and the issued client, never the old code.
                    registration = Some(Account {
                        client_id,
                        ..registration.unwrap_or_default()
                    });
                } else {
                    return Err(error.message);
                }
            }
        }
    }
    unreachable!()
}

struct ConnectError {
    message: String,
    retry_client: Option<String>,
}
impl From<String> for ConnectError {
    fn from(message: String) -> Self {
        Self {
            message,
            retry_client: None,
        }
    }
}
impl From<&str> for ConnectError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

fn connect_once(
    http: &reqwest::blocking::Client,
    host: &str,
    previous: Option<&Account>,
    operation: &Operation,
    remember: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<Account, ConnectError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|_| "ChatGPT 로그인 응답 포트를 열지 못했습니다")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "로그인 응답 준비 실패")?;
    let redirect = format!(
        "http://127.0.0.1:{}/auth/callback",
        listener
            .local_addr()
            .map_err(|_| "로그인 포트 조회 실패")?
            .port()
    );
    let (state, nonce, verifier) = (random()?, random()?, random()?);
    operation.check()?;
    open_browser(authorize_url(host, previous, &redirect, &state, &nonce, &verifier).as_str())?;
    let started = Instant::now();
    let (code, client_id) = loop {
        operation.check()?;
        if started.elapsed() > Duration::from_secs(240) {
            return Err("ChatGPT 로그인 시간이 초과되었습니다. 다시 연결해 주세요.".into());
        }
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if !peer.ip().is_loopback() {
                    continue;
                }
                stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
                stream.set_write_timeout(Some(Duration::from_secs(2))).ok();
                let mut request = Vec::new();
                let mut buffer = [0; 2048];
                while request.len() < 32768 {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => request.extend_from_slice(&buffer[..n]),
                    }
                    if request.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let parsed = parse_callback(&String::from_utf8_lossy(&request), &state, previous);
                let (status, message) = if parsed.is_ok() {
                    (
                        "200 OK",
                        "Sign-in received. Return to Niki to finish connecting.",
                    )
                } else {
                    (
                        "400 Bad Request",
                        "Sign-in was not accepted. Return to Niki, or retry in your original sign-in tab.",
                    )
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{message}",
                    message.len()
                );
                if let Ok(result) = parsed {
                    break result;
                }
                // An unrelated localhost request must not consume the pending attempt.
                if String::from_utf8_lossy(&request).contains(&format!("state={state}")) {
                    return Err(parsed.unwrap_err().into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(_) => return Err("ChatGPT 로그인 응답 수신 실패".into()),
        }
    };
    operation.check()?;
    remember(&client_id)?;
    let response = http
        .post(TOKEN)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("code", code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("resource", RESOURCE),
        ])
        .send()
        .map_err(|_| "ChatGPT 인증 교환 중 통신 오류가 발생했습니다. 다시 연결해 주세요.")?;
    let (status, value, request_id) = response_parts(response)?;
    if status != 200 {
        let code = value
            .get("error")
            .and_then(Value::as_str)
            .or_else(|| value.pointer("/error/code").and_then(Value::as_str));
        return Err(ConnectError {
            message: if code == Some("invalid_grant") {
                format!(
                    "ChatGPT 로그인 코드를 교환하지 못했습니다. 다시 연결해 주세요. (HTTP {status}, invalid_grant, 요청 {request_id})"
                )
            } else {
                api_error(status, &value, &request_id)
            },
            retry_client: (code == Some("invalid_grant")).then_some(client_id),
        });
    }
    let credentials = credentials(&value, None)?;
    let claims = identity(http, &credentials.id_token, &client_id, Some(&nonce))?;
    if previous.is_some_and(|a| !a.subject.is_empty() && a.subject != claims.sub) {
        return Err("선택한 ChatGPT 계정과 로그인한 계정이 다릅니다".into());
    }
    Ok(Account {
        client_id,
        subject: claims.sub,
        email: claims.email.unwrap_or_default(),
        welcome_seen: previous.is_some_and(|a| a.welcome_seen),
        credentials: Some(credentials),
    })
}

pub(super) fn access_token(
    http: &reqwest::blocking::Client,
    account: &mut Account,
) -> Result<String, String> {
    let old = account
        .credentials
        .as_ref()
        .ok_or("설정에서 ChatGPT를 다시 연결하세요")?
        .clone();
    if !has_plan(&old.scopes) {
        return Err(
            "ChatGPT 구독 사용 권한이 없습니다. 설정에서 구독 사용을 허용해 주세요.".into(),
        );
    }
    if old.expires_at > chrono::Utc::now().timestamp() + 60 {
        return Ok(old.access_token);
    }
    let response = http
        .post(TOKEN)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", account.client_id.as_str()),
            ("refresh_token", old.refresh_token.as_str()),
            ("resource", RESOURCE),
        ])
        .send()
        .map_err(|_| "ChatGPT 인증 갱신 중 통신 오류가 발생했습니다. 잠시 후 다시 시도하세요.")?;
    let status = response.status().as_u16();
    let mut bytes = Vec::new();
    response
        .take(2 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|_| "ChatGPT 인증 응답을 읽지 못했습니다")?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "ChatGPT 인증 응답 형식 오류")?;
    apply_refresh(account, status, &value, |token, client_id| {
        identity(http, token, client_id, None)
    })
}

fn apply_refresh(
    account: &mut Account,
    status: u16,
    value: &Value,
    validate: impl FnOnce(&str, &str) -> Result<Claims, String>,
) -> Result<String, String> {
    if status != 200 {
        let code = value
            .get("error")
            .and_then(Value::as_str)
            .or_else(|| value.pointer("/error/code").and_then(Value::as_str))
            .unwrap_or("");
        if matches!(
            code,
            "invalid_grant"
                | "invalid_refresh_token"
                | "token_expired"
                | "refresh_token_expired"
                | "refresh_token_invalidated"
                | "refresh_token_reused"
        ) {
            account.credentials = None;
        }
        return Err(api_error(status, &value, ""));
    }
    // A successful refresh can invalidate the old refresh token immediately. If its
    // replacement cannot be verified, require a new login instead of reusing it.
    let old = account
        .credentials
        .take()
        .ok_or("설정에서 ChatGPT를 다시 연결하세요")?;
    let next = credentials(value, Some(&old)).map_err(
        |_| "ChatGPT 인증 갱신 응답을 확인하지 못했습니다. 설정에서 다시 연결해 주세요.",
    )?;
    if value.get("id_token").is_some() {
        let claims = validate(&next.id_token, &account.client_id).map_err(
            |_| "갱신된 ChatGPT 인증을 검증하지 못했습니다. 설정에서 다시 연결해 주세요.",
        )?;
        if claims.sub != account.subject {
            return Err(
                "갱신된 ChatGPT 계정 정보가 일치하지 않습니다. 설정에서 다시 연결해 주세요.".into(),
            );
        }
    }
    let token = next.access_token.clone();
    let enabled = has_plan(&next.scopes);
    account.credentials = Some(next);
    if !enabled {
        return Err("ChatGPT 구독 사용 권한이 해제되었습니다. 설정에서 다시 허용해 주세요.".into());
    }
    Ok(token)
}

pub(super) fn revoke(client_id: &str, token: &str) -> bool {
    let Ok(http) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    else {
        return false;
    };
    for attempt in 0..2 {
        match http
            .post(REVOKE)
            .form(&[
                ("token", token),
                ("token_type_hint", "refresh_token"),
                ("client_id", client_id),
            ])
            .send()
        {
            Ok(response) if response.status().as_u16() == 200 => return true,
            Ok(response) if !response.status().is_server_error() => return false,
            _ if attempt == 0 => std::thread::sleep(Duration::from_millis(400)),
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    // This generated key signs test tokens only; it is not an app credential.
    fn signed(claims: &Value) -> String {
        let mut header = jsonwebtoken::Header::new(Algorithm::RS256);
        header.kid = Some("niki-test".into());
        jsonwebtoken::encode(
            &header,
            claims,
            &jsonwebtoken::EncodingKey::from_rsa_der(include_bytes!(
                "test-fixtures/rsa-private.der"
            )),
        )
        .unwrap()
    }
    fn valid_claims() -> Value {
        serde_json::json!({"sub":"person", "email":"test@example.invalid", "iss":ISSUER, "aud":"oaiapp_test", "iat":chrono::Utc::now().timestamp(), "exp":chrono::Utc::now().timestamp()+600,"nonce":"login-nonce"})
    }
    #[test]
    fn verifies_signed_identity_and_rejects_wrong_signature_identity_and_nonce() {
        let jwks: JwkSet = serde_json::from_str(include_str!("test-fixtures/jwks.json")).unwrap();
        let claims = valid_claims();
        assert_eq!(
            validate_identity(&signed(&claims), &jwks, "oaiapp_test", Some("login-nonce"))
                .unwrap()
                .sub,
            "person"
        );
        for (key, bad) in [
            ("iss", serde_json::json!("https://attacker.invalid")),
            ("aud", serde_json::json!("other-client")),
            ("nonce", serde_json::json!("old-nonce")),
            ("sub", serde_json::json!("")),
            ("exp", serde_json::json!(0)),
            (
                "iat",
                serde_json::json!(chrono::Utc::now().timestamp() + 600),
            ),
            ("azp", serde_json::json!("other-client")),
        ] {
            let mut changed = claims.clone();
            changed[key] = bad;
            assert!(
                validate_identity(&signed(&changed), &jwks, "oaiapp_test", Some("login-nonce"))
                    .is_err(),
                "Accepted invalid {key}"
            );
        }
        let mut multiple = claims.clone();
        multiple["aud"] = serde_json::json!(["oaiapp_test", "another"]);
        assert!(validate_identity(&signed(&multiple), &jwks, "oaiapp_test", None).is_err());
        multiple["azp"] = "oaiapp_test".into();
        assert!(validate_identity(&signed(&multiple), &jwks, "oaiapp_test", None).is_ok());
        let token = signed(&claims);
        let mut parts: Vec<_> = token.split('.').map(String::from).collect();
        parts[1] = URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&serde_json::json!({"sub":"forged"})).unwrap());
        assert!(validate_identity(&parts.join("."), &jwks, "oaiapp_test", None).is_err());
        let unsigned = format!(
            "{}.{}.signature",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"none","kid":"niki-test"}"#),
            parts[1]
        );
        assert!(validate_identity(&unsigned, &jwks, "oaiapp_test", None).is_err());
    }
    #[test]
    fn refresh_rotates_atomically_and_never_reuses_a_consumed_token() {
        let original = Account {
            client_id: "oaiapp_test".into(),
            subject: "person".into(),
            credentials: Some(Credentials {
                access_token: "old-access".into(),
                refresh_token: "old-refresh".into(),
                id_token: "old-id".into(),
                scopes: SCOPES.into(),
                expires_at: 0,
            }),
            ..Default::default()
        };
        let value = serde_json::json!({"token_type":"Bearer","access_token":"new-access","refresh_token":"new-refresh","expires_in":3600,"scope":SCOPES});
        let mut account = original.clone();
        assert_eq!(
            apply_refresh(&mut account, 200, &value, |_, _| unreachable!()).unwrap(),
            "new-access"
        );
        assert_eq!(
            account.credentials.as_ref().unwrap().refresh_token,
            "new-refresh"
        );
        assert_eq!(account.credentials.as_ref().unwrap().id_token, "old-id");
        assert!(account.credentials.as_ref().unwrap().expires_at > chrono::Utc::now().timestamp());
        let mut failed_id = value.clone();
        failed_id["id_token"] = "invalid-or-unverifiable".into();
        assert!(
            apply_refresh(&mut account, 200, &failed_id, |_, _| Err(
                "network or invalid signature".into()
            ))
            .is_err()
        );
        assert!(account.credentials.is_none());
        account = original.clone();
        let mut incomplete = value.clone();
        incomplete.as_object_mut().unwrap().remove("refresh_token");
        assert!(apply_refresh(&mut account, 200, &incomplete, |_, _| unreachable!()).is_err());
        assert!(account.credentials.is_none());
        account = original.clone();
        assert!(
            apply_refresh(
                &mut account,
                503,
                &serde_json::json!({"error":"unavailable"}),
                |_, _| unreachable!()
            )
            .is_err()
        );
        assert_eq!(
            account.credentials.as_ref().unwrap().refresh_token,
            "old-refresh"
        );
        assert!(
            apply_refresh(
                &mut account,
                400,
                &serde_json::json!({"error":"invalid_grant"}),
                |_, _| unreachable!()
            )
            .is_err()
        );
        assert!(account.credentials.is_none());
        account = original;
        let mut revoked = value;
        revoked["scope"] = "openid profile".into();
        assert!(apply_refresh(&mut account, 200, &revoked, |_, _| unreachable!()).is_err());
        assert!(!has_plan(&account.credentials.as_ref().unwrap().scopes));
    }
    #[test]
    fn pkce_matches_rfc7636_vector() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn callback_rejects_cross_account_replay_duplicates_and_denial() {
        let request = |query: &str| format!("GET /auth/callback?{query} HTTP/1.1\r\n\r\n");
        assert_eq!(
            parse_callback(&request("code=abc&state=s&client_id=oaiapp_new"), "s", None).unwrap(),
            ("abc".into(), "oaiapp_new".into())
        );
        for query in [
            "code=abc&state=bad&client_id=oaiapp_new",
            "code=abc&state=s&state=s&client_id=oaiapp_new",
            "code=abc&state=s",
            "code=abc&state=s&client_id=dynamic_agent_client",
            "error=access_denied&state=s",
        ] {
            assert!(parse_callback(&request(query), "s", None).is_err());
        }
        let previous = Account {
            client_id: "oaiapp_saved".into(),
            ..Default::default()
        };
        assert_eq!(
            parse_callback(&request("code=abc&state=s"), "s", Some(&previous))
                .unwrap()
                .1,
            "oaiapp_saved"
        );
        assert!(
            parse_callback(
                &request("code=abc&state=s&client_id=oaiapp_other"),
                "s",
                Some(&previous)
            )
            .is_err()
        );
    }
    #[test]
    fn reauthorization_retains_registration_and_host() {
        let previous = Account {
            client_id: "oaiapp_saved".into(),
            ..Default::default()
        };
        let url = authorize_url(
            "urn:uuid:host",
            Some(&previous),
            "http://127.0.0.1:4321/auth/callback",
            "state",
            "nonce",
            "verifier",
        );
        let pairs: HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(pairs.get("client_id").unwrap(), "oaiapp_saved");
        assert_eq!(pairs.get("ext_agent_host_id").unwrap(), "urn:uuid:host");
        assert!(!pairs.contains_key("agent_name_hint"));
        assert!(!pairs.contains_key("client_secret"));
    }
    #[test]
    fn errors_do_not_echo_provider_content_or_credentials() {
        let message = api_error(
            429,
            &serde_json::json!({"error":{"code":"subscription_sharing_usage_limit_exceeded", "message":"SECRET DIARY AND TOKEN"}}),
            "req_123",
        );
        assert!(message.contains("사용량 한도"));
        assert!(message.contains("req_123"));
        assert!(!message.contains("SECRET"));
    }
}
