use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::io::Write;

const IDENTITY: &[u8] = b"niki-chatgpt-credentials-v1";

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct Credentials {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub expires_at: i64,
    pub scopes: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct Account {
    pub client_id: String,
    pub subject: String,
    pub email: String,
    pub welcome_seen: bool,
    pub credentials: Option<Credentials>,
}

#[derive(Default, Serialize, Deserialize)]
pub(super) struct Accounts {
    pub active: Option<String>,
    pub accounts: Vec<Account>,
    /// Issued registration awaiting verified identity; never exposed as an active account.
    #[serde(default)]
    pub pending_client_id: Option<String>,
}

pub(super) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options
            .open(&temp)
            .map_err(|_| "ChatGPT 설정 파일을 만들지 못했습니다")?;
        file.write_all(bytes)
            .map_err(|_| "ChatGPT 설정을 기록하지 못했습니다")?;
        file.sync_all()
            .map_err(|_| "ChatGPT 설정을 저장하지 못했습니다")?;
        drop(file);
        fs::rename(&temp, path).map_err(|_| "ChatGPT 설정을 교체하지 못했습니다")
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.map_err(String::from)
}

pub(super) fn host_id(dir: &Path) -> Result<String, String> {
    let path = dir.join("chatgpt-host-id");
    if path.exists() {
        let id = fs::read_to_string(path).map_err(|_| "ChatGPT 기기 정보를 읽지 못했습니다")?;
        let uuid = id
            .strip_prefix("urn:uuid:")
            .ok_or("ChatGPT 기기 정보가 손상되었습니다")?;
        uuid::Uuid::parse_str(uuid).map_err(|_| "ChatGPT 기기 정보가 손상되었습니다")?;
        return Ok(id);
    }
    let id = format!("urn:uuid:{}", uuid::Uuid::new_v4());
    atomic_write(&path, id.as_bytes())?;
    Ok(id)
}

fn storage_key(dir: &Path, create: bool) -> Result<[u8; 32], String> {
    let host = host_id(dir)?;
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let entry = keyring::Entry::new("Niki ChatGPT", &host)
            .map_err(|_| "기기 자격 증명 저장소를 열지 못했습니다")?;
        match entry.get_password() {
            Ok(value) => STANDARD
                .decode(value)
                .ok()
                .and_then(|v| v.try_into().ok())
                .ok_or("ChatGPT 보호 키가 손상되었습니다".into()),
            Err(keyring::Error::NoEntry) if create => {
                let key = crate::sync_crypto::generate_key()?;
                entry
                    .set_password(&STANDARD.encode(key))
                    .map_err(|_| "ChatGPT 보호 키를 기기 자격 증명 저장소에 저장하지 못했습니다")?;
                Ok(key)
            }
            Err(_) => Err(
                "ChatGPT 보호 키를 읽지 못했습니다. 이 기기의 자격 증명 저장소를 확인하세요."
                    .into(),
            ),
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (host, create);
        Err("ChatGPT 연결은 Windows와 macOS에서 지원합니다".into())
    }
}

pub(super) fn load(dir: &Path) -> Result<Accounts, String> {
    let path = dir.join("chatgpt-credentials.enc");
    if !path.exists() {
        return Ok(Accounts::default());
    }
    let encrypted = fs::read(path).map_err(|_| "ChatGPT 연결 정보를 읽지 못했습니다")?;
    let key = storage_key(dir, false)?;
    decode(&key, &encrypted)
}

fn decode(key: &[u8; 32], encrypted: &[u8]) -> Result<Accounts, String> {
    let plain = crate::sync_crypto::decrypt(key, IDENTITY, encrypted)
        .map_err(|_| "ChatGPT 연결 정보를 해독하지 못했습니다")?;
    serde_json::from_slice(&plain).map_err(|_| "ChatGPT 연결 정보가 손상되었습니다".into())
}

pub(super) fn save(dir: &Path, accounts: &Accounts) -> Result<(), String> {
    let key = storage_key(dir, !dir.join("chatgpt-credentials.enc").exists())?;
    let plain = serde_json::to_vec(accounts).map_err(|_| "ChatGPT 연결 정보 직렬화 실패")?;
    let encrypted = crate::sync_crypto::encrypt(&key, IDENTITY, &plain)?;
    atomic_write(&dir.join("chatgpt-credentials.enc"), &encrypted)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_are_authenticated_and_never_plaintext() {
        let key = [42; 32];
        let credentials = Credentials {
            access_token: "private-access-token".into(),
            refresh_token: "private-refresh-token".into(),
            ..Default::default()
        };
        let accounts = Accounts {
            pending_client_id: None,
            active: Some("client".into()),
            accounts: vec![Account {
                credentials: Some(credentials),
                ..Default::default()
            }],
        };
        let plain = serde_json::to_vec(&accounts).unwrap();
        let mut encrypted = crate::sync_crypto::encrypt(&key, IDENTITY, &plain).unwrap();
        assert!(!String::from_utf8_lossy(&encrypted).contains("private-access-token"));
        assert_eq!(
            decode(&key, &encrypted).unwrap().accounts[0]
                .credentials
                .as_ref()
                .unwrap()
                .refresh_token,
            "private-refresh-token"
        );
        assert!(decode(&[41; 32], &encrypted).is_err());
        let n = encrypted.len();
        encrypted[n - 1] ^= 1;
        assert!(decode(&key, &encrypted).is_err());
    }
}
