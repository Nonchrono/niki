//! Chunked media transfer. The returned manifest must be stored in an authenticated revision.

use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

const CHUNK_SIZE: usize = 4 * 1024 * 1024;
const MAX_MEDIA: usize = 100_000_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    pub object_id: Uuid,
    pub file_id: String,
    pub bytes: usize,
    pub sha256: [u8; 32],
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub bytes: usize,
    pub sha256: [u8; 32],
    pub chunks: Vec<Chunk>,
}

pub trait BlobStore {
    fn put(&self, object_id: Uuid, bytes: &[u8]) -> Result<String, String>;
    fn get(&self, object_id: Uuid, file_id: &str) -> Result<Vec<u8>, String>;
}

pub struct DriveBlobs<'a> {
    pub client: &'a crate::drive_transport::DriveClient,
    pub token: &'a str,
    pub key: &'a [u8; 32],
}
impl BlobStore for DriveBlobs<'_> {
    fn put(&self, object_id: Uuid, bytes: &[u8]) -> Result<String, String> {
        let file = self
            .client
            .upload_encrypted(self.token, object_id, self.key, bytes)?;
        if file.name != object_id.to_string() {
            return Err("Drive 객체 이름 불일치".into());
        }
        Ok(file.id)
    }
    fn get(&self, object_id: Uuid, file_id: &str) -> Result<Vec<u8>, String> {
        self.client
            .download_decrypted(self.token, file_id, object_id, self.key)
    }
}

pub fn hash(bytes: &[u8]) -> [u8; 32] {
    digest(&SHA256, bytes)
        .as_ref()
        .try_into()
        .expect("SHA-256 length")
}

pub fn upload(store: &impl BlobStore, bytes: &[u8]) -> Result<Manifest, String> {
    if bytes.len() > MAX_MEDIA {
        return Err("첨부 크기 한도 초과".into());
    }
    let mut chunks = Vec::new();
    for part in bytes.chunks(CHUNK_SIZE) {
        let object_id = Uuid::new_v4();
        let file_id = store.put(object_id, part)?;
        chunks.push(Chunk {
            object_id,
            file_id,
            bytes: part.len(),
            sha256: hash(part),
        });
    }
    Ok(Manifest {
        version: 1,
        bytes: bytes.len(),
        sha256: hash(bytes),
        chunks,
    })
}

pub fn restore(store: &impl BlobStore, manifest: &Manifest) -> Result<Vec<u8>, String> {
    if manifest.version != 1
        || manifest.bytes > MAX_MEDIA
        || manifest.chunks.len() != manifest.bytes.div_ceil(CHUNK_SIZE)
    {
        return Err("잘못된 첨부 목록".into());
    }
    let mut object_ids = HashSet::new();
    let mut file_ids = HashSet::new();
    let mut result = Vec::with_capacity(manifest.bytes);
    for (i, chunk) in manifest.chunks.iter().enumerate() {
        let expected = (manifest.bytes - i * CHUNK_SIZE).min(CHUNK_SIZE);
        if chunk.bytes != expected
            || chunk.file_id.is_empty()
            || !object_ids.insert(chunk.object_id)
            || !file_ids.insert(&chunk.file_id)
        {
            return Err("잘못된 첨부 조각".into());
        }
        let bytes = store.get(chunk.object_id, &chunk.file_id)?;
        if bytes.len() != expected || hash(&bytes) != chunk.sha256 {
            return Err("첨부 조각 손상".into());
        }
        result.extend_from_slice(&bytes);
    }
    if hash(&result) != manifest.sha256 {
        return Err("첨부 손상".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashMap};
    #[derive(Default)]
    struct Fake(RefCell<HashMap<String, Vec<u8>>>);
    impl BlobStore for Fake {
        fn put(&self, id: Uuid, bytes: &[u8]) -> Result<String, String> {
            let id = id.to_string();
            self.0.borrow_mut().insert(id.clone(), bytes.to_vec());
            Ok(id)
        }
        fn get(&self, _: Uuid, id: &str) -> Result<Vec<u8>, String> {
            self.0.borrow().get(id).cloned().ok_or("missing".into())
        }
    }
    #[test]
    fn restores_large_media_and_rejects_missing_or_corrupt_chunks() {
        let store = Fake::default();
        let data = vec![71; CHUNK_SIZE + 123];
        let manifest = upload(&store, &data).unwrap();
        assert_eq!(manifest.chunks.len(), 2);
        assert_eq!(restore(&store, &manifest).unwrap(), data);
        store
            .0
            .borrow_mut()
            .get_mut(&manifest.chunks[1].file_id)
            .unwrap()[0] ^= 1;
        assert!(restore(&store, &manifest).is_err());
        store.0.borrow_mut().remove(&manifest.chunks[0].file_id);
        assert!(restore(&store, &manifest).is_err());
    }
    #[test]
    fn empty_media_and_duplicate_ids_are_checked() {
        let store = Fake::default();
        let empty = upload(&store, b"").unwrap();
        assert!(restore(&store, &empty).unwrap().is_empty());
        let mut m = upload(&store, &vec![1; CHUNK_SIZE + 1]).unwrap();
        m.chunks[1].object_id = m.chunks[0].object_id;
        assert!(restore(&store, &m).is_err());
    }
}
