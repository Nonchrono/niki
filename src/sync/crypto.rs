//! Authenticated encryption for Google Drive sync objects.

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD},
};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};

const MAGIC: &[u8; 8] = b"NIKISY01";
const NONCE_LEN: usize = 12;
const MAX_OBJECT: usize = 250 * 1024 * 1024;

pub fn generate_key() -> Result<[u8; 32], &'static str> {
    let mut key = [0u8; 32];
    SystemRandom::new()
        .fill(&mut key)
        .map_err(|_| "난수 생성 실패")?;
    Ok(key)
}

pub fn encode_recovery_key(key: &[u8; 32]) -> String {
    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
    hex.as_bytes()
        .chunks(8)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn decode_recovery_key(input: &str) -> Result<[u8; 32], String> {
    let trimmed = input.trim();
    let stripped = trimmed
        .strip_prefix("NIKI-")
        .or_else(|| trimmed.strip_prefix("niki-"))
        .unwrap_or(trimmed);
    let compact: String = stripped
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    if compact.len() == 64 && compact.bytes().all(|b| b.is_ascii_hexdigit()) {
        let mut key = [0u8; 32];
        for (i, slot) in key.iter_mut().enumerate() {
            *slot = u8::from_str_radix(&compact[i * 2..i * 2 + 2], 16)
                .map_err(|_| "복구 키 형식이 올바르지 않습니다".to_string())?;
        }
        return Ok(key);
    }
    if let Ok(bytes) = URL_SAFE_NO_PAD
        .decode(stripped.as_bytes())
        .or_else(|_| B64.decode(stripped.as_bytes()))
        && bytes.len() == 32
    {
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }
    Err("복구 키 형식이 올바르지 않습니다 (64자리 16진수 복구 키를 입력하세요)".into())
}

/// The caller supplies the canonical object identity, which is authenticated but not stored.
pub fn encrypt(key: &[u8; 32], identity: &[u8], plain: &[u8]) -> Result<Vec<u8>, &'static str> {
    if plain.len() > MAX_OBJECT || identity.len() > 1024 {
        return Err("동기화 데이터 크기 초과");
    }
    let mut nonce = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| "난수 생성 실패")?;
    let key = aead::LessSafeKey::new(
        aead::UnboundKey::new(&aead::AES_256_GCM, key).map_err(|_| "키 오류")?,
    );
    let mut aad = MAGIC.to_vec();
    aad.extend_from_slice(identity);
    let mut body = plain.to_vec();
    key.seal_in_place_append_tag(
        aead::Nonce::assume_unique_for_key(nonce),
        aead::Aad::from(&aad),
        &mut body,
    )
    .map_err(|_| "암호화 실패")?;
    let mut output = MAGIC.to_vec();
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&body);
    Ok(output)
}

pub fn decrypt(key: &[u8; 32], identity: &[u8], object: &[u8]) -> Result<Vec<u8>, &'static str> {
    if object.len() < MAGIC.len() + NONCE_LEN + aead::AES_256_GCM.tag_len()
        || object.len() > MAX_OBJECT + MAGIC.len() + NONCE_LEN + aead::AES_256_GCM.tag_len()
        || identity.len() > 1024
        || &object[..MAGIC.len()] != MAGIC
    {
        return Err("잘못된 동기화 데이터");
    }
    let nonce: [u8; NONCE_LEN] = object[MAGIC.len()..MAGIC.len() + NONCE_LEN]
        .try_into()
        .map_err(|_| "잘못된 nonce")?;
    let key = aead::LessSafeKey::new(
        aead::UnboundKey::new(&aead::AES_256_GCM, key).map_err(|_| "키 오류")?,
    );
    let mut aad = MAGIC.to_vec();
    aad.extend_from_slice(identity);
    let mut body = object[MAGIC.len() + NONCE_LEN..].to_vec();
    let plain = key
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(&aad),
            &mut body,
        )
        .map_err(|_| "인증 실패")?;
    Ok(plain.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_and_reject_tampering_or_wrong_identity() {
        let key = generate_key().unwrap();
        let mut encrypted = encrypt(&key, b"entry/1/revision/2", b"private diary").unwrap();
        assert_eq!(
            decrypt(&key, b"entry/1/revision/2", &encrypted).unwrap(),
            b"private diary"
        );
        assert!(decrypt(&key, b"entry/other", &encrypted).is_err());
        let last = encrypted.len() - 1;
        encrypted[last] ^= 1;
        assert!(decrypt(&key, b"entry/1/revision/2", &encrypted).is_err());
    }
    #[test]
    fn fresh_nonce_changes_ciphertext() {
        let key = generate_key().unwrap();
        assert_ne!(
            encrypt(&key, b"object", b"same").unwrap(),
            encrypt(&key, b"object", b"same").unwrap()
        );
    }
    #[test]
    fn recovery_key_round_trips_and_rejects_invalid_input() {
        let key = generate_key().unwrap();
        let encoded = encode_recovery_key(&key);
        assert_eq!(encoded.len(), 71);
        assert_eq!(decode_recovery_key(&encoded).unwrap(), key);
        assert_eq!(
            decode_recovery_key(&encoded.to_ascii_uppercase()).unwrap(),
            key
        );
        let b64 = URL_SAFE_NO_PAD.encode(key);
        assert_eq!(decode_recovery_key(&b64).unwrap(), key);
        assert!(decode_recovery_key("short-key").is_err());
    }
}
