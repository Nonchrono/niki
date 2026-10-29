//! Drive appDataFolder transport.

use serde::Deserialize;
use std::{io::Read, time::Duration};
use uuid::Uuid;

const LIMIT: usize = 5 * 1024 * 1024;
const MAGIC: &[u8] = b"NIKISY01";
pub const ACCOUNT_KEY_UUID: Uuid = Uuid::from_u128(0x4e494b49_4b45_5931_0000_000000000001);
pub const ACCOUNT_ENVELOPE_KEY: [u8; 32] = *b"NikiJournalGoogleDriveAutoKeyV01";

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
        self.client.list(self.token)
    }
    fn start_cursor(&self) -> Result<String, String> {
        self.client.start_cursor(self.token)
    }
    fn changes(&self, cursor: &str) -> Result<ChangeBatch, String> {
        self.client.changes(self.token, cursor)
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
    fn upload(&self, token: &str, object_id: Uuid, ciphertext: &[u8]) -> Result<DriveFile, String> {
        if ciphertext.len() > LIMIT || !ciphertext.starts_with(MAGIC) {
            return Err("암호화된 파일만 5 MiB 이하로 업로드할 수 있습니다".into());
        }
        let boundary = format!("niki_{}", Uuid::new_v4().simple());
        let metadata =
            serde_json::json!({"name": object_id.to_string(), "parents": ["appDataFolder"]});
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
    pub fn delete(&self, token: &str, file_id: &str) -> Result<(), String> {
        if file_id.is_empty()
            || file_id.len() > 256
            || !file_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err("잘못된 Drive 파일 ID".into());
        }
        self.http
            .delete(format!("{}/files/{file_id}", self.base))
            .bearer_auth(token)
            .send()
            .map_err(|_| "Drive 파일 삭제 요청 실패")?
            .error_for_status()
            .map_err(|_| "Drive 파일 삭제 실패")?;
        Ok(())
    }
    /// Automatically synchronizes the encryption key across devices via Google Drive appDataFolder.
    pub fn ensure_cloud_sync_key(
        &self,
        token: &str,
        local_key: Option<[u8; 32]>,
    ) -> Result<([u8; 32], bool), String> {
        let files = self.list(token)?;
        let key_name = ACCOUNT_KEY_UUID.to_string();
        let key_file = files.iter().find(|f| f.name == key_name);
        let rev_files: Vec<(&DriveFile, Uuid)> = files
            .iter()
            .filter_map(|f| crate::sync_engine::parse_revision_uuid(&f.name).map(|u| (f, u)))
            .collect();

        if let Some(kf) = key_file
            && let Ok(plain) = self.download_decrypted(
                token,
                &kf.id,
                ACCOUNT_KEY_UUID,
                &ACCOUNT_ENVELOPE_KEY,
            )
            && let Ok(cloud_key) = <[u8; 32]>::try_from(plain.as_slice())
        {
            let mut cleaned_legacy = false;
            for (rf, ruuid) in &rev_files {
                if self.download_decrypted(token, &rf.id, *ruuid, &cloud_key).is_err() {
                    let _ = self.delete(token, &rf.id);
                    cleaned_legacy = true;
                }
            }
            let changed = local_key != Some(cloud_key) || cleaned_legacy;
            return Ok((cloud_key, changed));
        }

        let mut chosen_key = local_key.unwrap_or(ACCOUNT_ENVELOPE_KEY);
        let mut reset_needed = false;

        if let Some((first_rf, first_uuid)) = rev_files.first() {
            if let Some(lk) = local_key
                && self.download_decrypted(token, &first_rf.id, *first_uuid, &lk).is_ok()
            {
                chosen_key = lk;
            } else if self
                .download_decrypted(token, &first_rf.id, *first_uuid, &ACCOUNT_ENVELOPE_KEY)
                .is_ok()
            {
                chosen_key = ACCOUNT_ENVELOPE_KEY;
                reset_needed = local_key != Some(ACCOUNT_ENVELOPE_KEY);
            } else {
                for f in &files {
                    let _ = self.delete(token, &f.id);
                }
                reset_needed = true;
            }
        }

        let _ = self.upload_encrypted(
            token,
            ACCOUNT_KEY_UUID,
            &ACCOUNT_ENVELOPE_KEY,
            &chosen_key,
        );
        Ok((chosen_key, reset_needed))
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
