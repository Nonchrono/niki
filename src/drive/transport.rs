//! Drive appDataFolder transport.

use serde::Deserialize;
use std::{io::Read, time::Duration};
use uuid::Uuid;

const LIMIT: usize = 5 * 1024 * 1024;
const MAGIC: &[u8] = b"NIKISY01";
pub const ACCOUNT_KEY_UUID: Uuid = Uuid::from_u128(0x4e494b49_4b45_5931_0000_000000000001);
pub const ACCOUNT_ENVELOPE_KEY: [u8; 32] = *b"NikiJournalGoogleDriveAutoKeyV01";

/// Derives a deterministic 256-bit encryption key from the user's Google account email.
/// This allows any device authenticated with the same Google account to automatically
/// decrypt and sync entries seamlessly without manual recovery key entry.
pub fn derive_account_sync_key(email: &str) -> [u8; 32] {
    let normalized = email.trim().to_ascii_lowercase();
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    ctx.update(b"NikiJournal::GoogleDriveSyncKey::v1::");
    ctx.update(normalized.as_bytes());
    let d = ctx.finish();
    let mut key = [0u8; 32];
    key.copy_from_slice(d.as_ref());
    key
}

// Namespaced files are ignored by legacy clients, including their cleanup pass.
pub const V2_PREFIX: &str = "niki-v2-";
pub const V2_KEY_UUID: Uuid = Uuid::from_u128(0x4e494b49_4b45_5932_0000_000000000002);
pub fn v2_name(id: Uuid) -> String { format!("{V2_PREFIX}{id}") }
pub fn decode_v2_name(name: &str) -> Option<String> {
    name.strip_prefix(V2_PREFIX).and_then(|part| Uuid::parse_str(part).ok()).map(|id| id.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct DriveFile {
    pub id: String,
    pub name: String,
}
#[derive(Deserialize)]
struct Page {
    files: Vec<DriveFile>,
    #[serde(rename = "nextPageToken")]
    next: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct DriveChange {
    #[serde(rename = "fileId")]
    pub file_id: String,
    #[serde(default)]
    pub removed: bool,
    pub file: Option<DriveFile>,
}

#[derive(Deserialize)]
struct ChangePage {
    #[serde(default)]
    changes: Vec<DriveChange>,
    #[serde(rename = "nextPageToken")]
    next: Option<String>,
    #[serde(rename = "newStartPageToken")]
    new_start: Option<String>,
}

#[derive(Deserialize)]
struct StartPage {
    #[serde(rename = "startPageToken")]
    token: String,
}

/// The caller must persist `next_cursor` only after every change is applied successfully.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeBatch {
    pub changes: Vec<DriveChange>,
    pub next_cursor: String,
}

pub trait RemoteTransport: crate::sync_media::BlobStore {
    fn list(&self) -> Result<Vec<DriveFile>, String>;
    fn start_cursor(&self) -> Result<String, String>;
    fn changes(&self, cursor: &str) -> Result<ChangeBatch, String>;
}

impl RemoteTransport for crate::sync_media::DriveBlobs<'_> {
    fn list(&self) -> Result<Vec<DriveFile>, String> {
        let files = self.client.list(self.token)?;
        Ok(if self.v2 { files.into_iter().filter_map(|mut f| { f.name=decode_v2_name(&f.name)?; Some(f) }).collect() } else { files })
    }
    fn start_cursor(&self) -> Result<String, String> {
        self.client.start_cursor(self.token)
    }
    fn changes(&self, cursor: &str) -> Result<ChangeBatch, String> {
        let mut batch=self.client.changes(self.token, cursor)?;
        if self.v2 {
            for change in &mut batch.changes {
                if let Some(file) = &mut change.file {
                    file.name=decode_v2_name(&file.name).unwrap_or_default();
                }
            }
        }
        Ok(batch)
    }
}

pub struct DriveClient {
    http: reqwest::blocking::Client,
    base: String,
    upload_base: String,
}
impl DriveClient {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            http: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(60))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| "HTTP 초기화 실패")?,
            base: "https://www.googleapis.com/drive/v3".into(),
            upload_base: "https://www.googleapis.com/upload/drive/v3".into(),
        })
    }
    pub fn list(&self, token: &str) -> Result<Vec<DriveFile>, String> {
        let mut files = Vec::new();
        let mut page = None::<String>;
        for _ in 0..1000 {
            let mut request = self
                .http
                .get(format!("{}/files", self.base))
                .bearer_auth(token)
                .query(&[
                    ("spaces", "appDataFolder"),
                    ("fields", "nextPageToken,files(id,name)"),
                    ("pageSize", "1000"),
                ]);
            if let Some(ref p) = page {
                request = request.query(&[("pageToken", p)]);
            }
            let response = request
                .send()
                .map_err(|_| "Drive 목록 요청 실패")?
                .error_for_status()
                .map_err(|_| "Drive 목록 조회 실패")?;
            let bytes = read_limited(response, LIMIT)?;
            let result: Page =
                serde_json::from_slice(&bytes).map_err(|_| "Drive 목록 응답 오류")?;
            files.extend(result.files);
            if files.len() > 100_000 {
                return Err("Drive 파일 수 한도 초과".into());
            }
            match result.next {
                Some(next) if !next.is_empty() => page = Some(next),
                _ => return Ok(files),
            }
        }
        Err("Drive 목록 페이지 한도 초과".into())
    }
    /// Take this cursor before the initial full listing, then process changes from it.
    pub fn start_cursor(&self, token: &str) -> Result<String, String> {
        let response = self
            .http
            .get(format!("{}/changes/startPageToken", self.base))
            .bearer_auth(token)
            .send()
            .map_err(|_| "Drive 시작 위치 요청 실패")?
            .error_for_status()
            .map_err(|_| "Drive 시작 위치 조회 실패")?;
        let page: StartPage = serde_json::from_slice(&read_limited(response, LIMIT)?)
            .map_err(|_| "Drive 시작 위치 응답 오류")?;
        if page.token.is_empty() || page.token.len() > 4096 {
            return Err("잘못된 Drive 시작 위치".into());
        }
        Ok(page.token)
    }

    pub fn changes(&self, token: &str, cursor: &str) -> Result<ChangeBatch, String> {
        if cursor.is_empty() || cursor.len() > 4096 {
            return Err("잘못된 Drive 진행 위치".into());
        }
        let mut page_token = cursor.to_owned();
        let mut changes = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1000 {
            if !seen.insert(page_token.clone()) {
                return Err("Drive 페이지 반복 오류".into());
            }
            let response = self
                .http
                .get(format!("{}/changes", self.base))
                .bearer_auth(token)
                .query(&[
                    ("pageToken", page_token.as_str()),
                    ("spaces", "appDataFolder"),
                    ("pageSize", "1000"),
                    (
                        "fields",
                        "nextPageToken,newStartPageToken,changes(fileId,removed,file(id,name))",
                    ),
                ])
                .send()
                .map_err(|_| "Drive 변경 요청 실패")?
                .error_for_status()
                .map_err(|_| "Drive 변경 조회 실패")?;
            let page: ChangePage = serde_json::from_slice(&read_limited(response, LIMIT)?)
                .map_err(|_| "Drive 변경 응답 오류")?;
            changes.extend(page.changes);
            if changes.len() > 100_000 {
                return Err("Drive 변경 수 한도 초과".into());
            }
            if let Some(next) = page.next {
                if next.is_empty() || next.len() > 4096 {
                    return Err("잘못된 Drive 페이지".into());
                }
                page_token = next;
            } else {
                let next_cursor = page
                    .new_start
                    .filter(|p| !p.is_empty() && p.len() <= 4096)
                    .ok_or("Drive 최종 진행 위치 누락")?;
                return Ok(ChangeBatch {
                    changes,
                    next_cursor,
                });
            }
        }
        Err("Drive 변경 페이지 한도 초과".into())
    }
    pub fn upload_encrypted(
        &self,
        token: &str,
        object_id: Uuid,
        key: &[u8; 32],
        plain: &[u8],
    ) -> Result<DriveFile, String> {
        let ciphertext = crate::sync_crypto::encrypt(key, object_id.to_string().as_bytes(), plain)?;
        self.upload(token, object_id, &ciphertext)
    }
    pub fn upload_v2_encrypted(&self, token: &str, object_id: Uuid, key: &[u8; 32], plain: &[u8]) -> Result<DriveFile, String> {
        let ciphertext = crate::sync_crypto::encrypt(key, object_id.to_string().as_bytes(), plain)?;
        self.upload_named(token, &v2_name(object_id), &ciphertext)
    }
    fn upload(&self, token: &str, object_id: Uuid, ciphertext: &[u8]) -> Result<DriveFile, String> {
        self.upload_named(token, &object_id.to_string(), ciphertext)
    }
    fn upload_named(&self, token: &str, name: &str, ciphertext: &[u8]) -> Result<DriveFile, String> {
        if ciphertext.len() > LIMIT || !ciphertext.starts_with(MAGIC) {
            return Err("암호화된 파일만 5 MiB 이하로 업로드할 수 있습니다".into());
        }
        let boundary = format!("niki_{}", Uuid::new_v4().simple());
        let metadata =
            serde_json::json!({"name": name, "parents": ["appDataFolder"]});
        let mut body = format!("--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes();
        body.extend_from_slice(ciphertext);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let response = self
            .http
            .post(format!("{}/files", self.upload_base))
            .query(&[("uploadType", "multipart"), ("fields", "id,name")])
            .bearer_auth(token)
            .header(
                "Content-Type",
                format!("multipart/related; boundary={boundary}"),
            )
            .body(body)
            .send()
            .map_err(|_| "Drive 업로드 요청 실패")?
            .error_for_status()
            .map_err(|_| "Drive 업로드 실패")?;
        serde_json::from_slice(&read_limited(response, LIMIT)?)
            .map_err(|_| "Drive 업로드 응답 오류".into())
    }
    pub fn download_decrypted(
        &self,
        token: &str,
        file_id: &str,
        object_id: Uuid,
        key: &[u8; 32],
    ) -> Result<Vec<u8>, String> {
        let ciphertext = self.download(token, file_id)?;
        crate::sync_crypto::decrypt(key, object_id.to_string().as_bytes(), &ciphertext)
            .map_err(str::to_string)
    }
    fn download(&self, token: &str, file_id: &str) -> Result<Vec<u8>, String> {
        if file_id.is_empty()
            || file_id.len() > 256
            || !file_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err("잘못된 Drive 파일 ID".into());
        }
        let response = self
            .http
            .get(format!("{}/files/{file_id}", self.base))
            .query(&[("alt", "media")])
            .bearer_auth(token)
            .send()
            .map_err(|_| "Drive 다운로드 요청 실패")?
            .error_for_status()
            .map_err(|_| "Drive 다운로드 실패")?;
        let bytes = read_limited(response, LIMIT)?;
        if !bytes.starts_with(MAGIC) {
            return Err("암호화 형식이 아닙니다".into());
        }
        Ok(bytes)
    }
    /// The key is stored in the account's private Drive folder. Its wrapping key
    /// is part of the app, so this is account-protected, not end-to-end secrecy.
    pub fn account_v2_key(&self, token: &str) -> Result<[u8; 32], String> {
        let name=v2_name(V2_KEY_UUID);
        let find = |files: Vec<DriveFile>| -> Result<Option<DriveFile>, String> {
            let found: Vec<_>=files.into_iter().filter(|f| f.name==name).collect();
            if found.len()>1 { return Err("동기화 키가 여러 개 발견되어 자동 선택을 중단했습니다".into()); }
            Ok(found.into_iter().next())
        };
        let files=self.list(token)?;
        let has_v2_data=files.iter().any(|f| f.name.starts_with(V2_PREFIX) && f.name!=name);
        let file=match find(files)? {
            Some(file)=>file,
            None=>{
                if has_v2_data { return Err("동기화 키가 없지만 암호화된 기록이 남아 있어 새 키를 만들지 않았습니다".into()); }
                let key=crate::sync_crypto::generate_key().map_err(str::to_string)?;
                self.upload_v2_encrypted(token,V2_KEY_UUID,&ACCOUNT_ENVELOPE_KEY,&key)?;
                find(self.list(token)?)?.ok_or("생성한 동기화 키를 찾을 수 없습니다")?
            }
        };
        let bytes=self.download_decrypted(token,&file.id,V2_KEY_UUID,&ACCOUNT_ENVELOPE_KEY)?;
        bytes.try_into().map_err(|_| "동기화 키의 길이가 올바르지 않습니다".into())
    }
    /// Automatically synchronizes the encryption key across devices via Google Drive appDataFolder.
    pub fn ensure_cloud_sync_key(
        &self,
        token: &str,
        canonical_key: [u8; 32],
    ) -> Result<bool, String> {
        let files = self.list(token)?;
        let key_name = ACCOUNT_KEY_UUID.to_string();
        let key_file = files.iter().find(|f| f.name == key_name);
        let rev_files: Vec<(&DriveFile, Uuid)> = files
            .iter()
            .filter_map(|f| crate::sync_engine::parse_revision_uuid(&f.name).map(|u| (f, u)))
            .collect();

        if let Some(kf) = key_file {
            let plain = self.download_decrypted(
                token,
                &kf.id,
                ACCOUNT_KEY_UUID,
                &ACCOUNT_ENVELOPE_KEY,
            )?;
            if plain.as_slice() != canonical_key.as_slice() {
                return Err("동기화 키가 기존 원격 데이터의 키와 다릅니다. 원격 데이터는 변경하지 않았습니다".into());
            }
        } else if rev_files.is_empty() {
            self.upload_encrypted(token, ACCOUNT_KEY_UUID, &ACCOUNT_ENVELOPE_KEY, &canonical_key)?;
        }

        // A download or decryption error may be transient or indicate a different key.
        // Never delete remote revisions in either case.
        for (rf, ruuid) in rev_files {
            self.download_decrypted(token, &rf.id, ruuid, &canonical_key)?;
        }
        Ok(false)
    }
}

fn read_limited(reader: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    reader
        .take((limit + 1) as u64)
        .read_to_end(&mut result)
        .map_err(|_| "Drive 응답 읽기 실패")?;
    if result.len() > limit {
        return Err("Drive 응답 크기 한도 초과".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpListener, thread};
    #[test]
    fn derives_deterministic_account_key() {
        let k1 = derive_account_sync_key("test@example.com");
        let k2 = derive_account_sync_key("  TEST@example.com ");
        let k3 = derive_account_sync_key("other@example.com");
        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
        assert_ne!(k1, [0u8; 32]);
    }

    #[test]
    fn v2_names_do_not_match_legacy_revision_names() {
        let revision=crate::sync_engine::new_revision_uuid();
        let name=v2_name(revision);
        assert!(crate::sync_engine::parse_revision_uuid(&name).is_none());
        assert_eq!(decode_v2_name(&name).as_deref(), Some(revision.to_string().as_str()));
        assert!(decode_v2_name(&revision.to_string()).is_none());
    }

    #[test]
    fn enforces_size_and_requires_ciphertext() {
        assert!(read_limited(&b"12345"[..], 4).is_err());
        assert_eq!(read_limited(&b"1234"[..], 4).unwrap(), b"1234");
        let client = DriveClient::new().unwrap();
        assert!(
            client
                .upload("unused", Uuid::new_v4(), b"plaintext")
                .is_err()
        );
        assert!(client.download("unused", "../other").is_err());
    }

    #[test]
    #[ignore = "requires loopback socket access"]
    fn v2_missing_or_duplicate_key_never_creates_a_replacement() {
        for duplicate in [false, true] {
            let listener=TcpListener::bind("127.0.0.1:0").unwrap();
            let base=format!("http://{}",listener.local_addr().unwrap());
            let server=thread::spawn(move || {
                let (mut stream,_)=listener.accept().unwrap();let mut request=[0u8;4096];
                let n=stream.read(&mut request).unwrap();assert!(String::from_utf8_lossy(&request[..n]).starts_with("GET "));
                let name=if duplicate {v2_name(V2_KEY_UUID)} else {v2_name(crate::sync_engine::new_revision_uuid())};
                let body=if duplicate {format!("{{\"files\":[{{\"id\":\"a\",\"name\":\"{name}\"}},{{\"id\":\"b\",\"name\":\"{name}\"}}]}}")}
                    else {format!("{{\"files\":[{{\"id\":\"a\",\"name\":\"{name}\"}}]}}")};
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            });
            let mut client=DriveClient::new().unwrap();client.http=reqwest::blocking::Client::builder().no_proxy().build().unwrap();client.base=base.clone();client.upload_base=base;
            assert!(client.account_v2_key("test-token").is_err());server.join().unwrap();
        }
    }

    #[test]
    #[ignore = "requires loopback socket access"]
    fn v2_key_is_generated_once_and_read_on_another_device() {
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();
        let base=format!("http://{}",listener.local_addr().unwrap());
        let server=thread::spawn(move || {
            let mut stored=Vec::new();
            for i in 0..6 {
                let (mut stream,_)=listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut request=Vec::new();let mut buf=[0;4096];
                let end=loop {let n=stream.read(&mut buf).unwrap();assert!(n>0);request.extend_from_slice(&buf[..n]);if let Some(p)=request.windows(4).position(|w|w==b"\r\n\r\n"){break p+4;}};
                let header=String::from_utf8_lossy(&request[..end]).into_owned();
                let len=header.lines().find_map(|l|l.to_ascii_lowercase().strip_prefix("content-length: ").and_then(|x|x.parse::<usize>().ok())).unwrap_or(0);
                while request.len()<end+len {let n=stream.read(&mut buf).unwrap();assert!(n>0);request.extend_from_slice(&buf[..n]);}
                let body=match i {
                    0=>b"{\"files\":[]}".to_vec(),
                    1=>{assert!(header.starts_with("POST "));let payload=&request[end..];let start=payload.windows(MAGIC.len()).position(|w|w==MAGIC).unwrap();let stop=payload[start..].windows(9).position(|w|w==b"\r\n--niki_").unwrap()+start;stored=payload[start..stop].to_vec();format!("{{\"id\":\"key2\",\"name\":\"{}\"}}",v2_name(V2_KEY_UUID)).into_bytes()},
                    2|4=>format!("{{\"files\":[{{\"id\":\"key2\",\"name\":\"{}\"}}]}}",v2_name(V2_KEY_UUID)).into_bytes(),
                    _=>stored.clone(),
                };
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();stream.write_all(&body).unwrap();
            }
        });
        let mut client=DriveClient::new().unwrap();client.http=reqwest::blocking::Client::builder().no_proxy().build().unwrap();client.base=base.clone();client.upload_base=base;
        let first=client.account_v2_key("test-token").unwrap();
        let second=client.account_v2_key("test-token").unwrap();
        assert_eq!(first,second);assert_ne!(first,ACCOUNT_ENVELOPE_KEY);
        server.join().unwrap();
    }

    #[test]
    #[ignore = "requires loopback socket access"]
    fn key_mismatch_does_not_delete_remote_objects() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let ciphertext = crate::sync_crypto::encrypt(
            &ACCOUNT_ENVELOPE_KEY,
            ACCOUNT_KEY_UUID.to_string().as_bytes(),
            &[7u8; 32],
        ).unwrap();
        let server = thread::spawn(move || {
            for index in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 4096];
                let n = stream.read(&mut request).unwrap();
                let header = String::from_utf8_lossy(&request[..n]);
                assert!(header.starts_with("GET "), "unexpected remote mutation: {header}");
                let body = if index == 0 {
                    format!("{{\"files\":[{{\"id\":\"key1\",\"name\":\"{}\"}}]}}", ACCOUNT_KEY_UUID).into_bytes()
                } else { ciphertext.clone() };
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        let mut client = DriveClient::new().unwrap();
        client.http = reqwest::blocking::Client::builder().no_proxy().build().unwrap();
        client.base = base.clone();
        client.upload_base = base;
        let error = client.ensure_cloud_sync_key("test-token", [8u8; 32]).unwrap_err();
        assert!(error.contains("원격 데이터는 변경하지 않았습니다"));
        server.join().unwrap();
    }

    #[test]
    #[ignore = "requires loopback socket access"]
    fn mock_drive_upload_list_and_download() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let object_id = Uuid::new_v4();
        let key = [42u8; 32];
        let ciphertext =
            crate::sync_crypto::encrypt(&key, object_id.to_string().as_bytes(), b"private text")
                .unwrap();
        let remote_ciphertext = ciphertext.clone();
        let server = thread::spawn(move || {
            for i in 0..6 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 4096];
                let header_end = loop {
                    let n = stream.read(&mut buffer).unwrap();
                    assert_ne!(n, 0);
                    request.extend_from_slice(&buffer[..n]);
                    if let Some(p) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        break p + 4;
                    }
                };
                let header = String::from_utf8_lossy(&request[..header_end]).into_owned();
                assert!(
                    header
                        .to_ascii_lowercase()
                        .contains("authorization: bearer test-token")
                );
                let length = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|x| x.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                while request.len() < header_end + length {
                    let n = stream.read(&mut buffer).unwrap();
                    assert_ne!(n, 0);
                    request.extend_from_slice(&buffer[..n]);
                }
                let body = match i {
                    0 => {
                        assert!(
                            request
                                .windows(b"private text".len())
                                .all(|w| w != b"private text")
                        );
                        format!("{{\"id\":\"file1\",\"name\":\"{object_id}\"}}").into_bytes()
                    }
                    1 => format!("{{\"files\":[{{\"id\":\"file1\",\"name\":\"{object_id}\"}}]}}")
                        .into_bytes(),
                    2 => remote_ciphertext.clone(),
                    3 => b"{\"startPageToken\":\"cursor0\"}".to_vec(),
                    4 => {
                        assert!(
                            header.contains("pageToken=cursor0")
                                && header.contains("spaces=appDataFolder")
                        );
                        b"{\"nextPageToken\":\"cursor1\",\"changes\":[{\"fileId\":\"file1\",\"removed\":false,\"file\":{\"id\":\"file1\",\"name\":\"object\"}}]}".to_vec()
                    }
                    _ => {
                        assert!(header.contains("pageToken=cursor1"));
                        b"{\"newStartPageToken\":\"cursor2\",\"changes\":[{\"fileId\":\"file2\",\"removed\":true}]}".to_vec()
                    }
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        let mut client = DriveClient::new().unwrap();
        client.http = reqwest::blocking::Client::builder()
            .no_proxy()
            .build()
            .unwrap();
        client.base = base.clone();
        client.upload_base = base;
        assert_eq!(
            client
                .upload_encrypted("test-token", object_id, &key, b"private text")
                .unwrap()
                .id,
            "file1"
        );
        assert_eq!(client.list("test-token").unwrap().len(), 1);
        assert_eq!(
            client
                .download_decrypted("test-token", "file1", object_id, &key)
                .unwrap(),
            b"private text"
        );
        let cursor = client.start_cursor("test-token").unwrap();
        let batch = client.changes("test-token", &cursor).unwrap();
        assert_eq!(batch.next_cursor, "cursor2");
        assert_eq!(batch.changes.len(), 2);
        assert!(!batch.changes[0].removed && batch.changes[1].removed);
        server.join().unwrap();
    }
}
