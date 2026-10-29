use std::io::Read;

const MAX_FILE: usize = 100 * 1024 * 1024;
const MAX_TOTAL: usize = 250 * 1024 * 1024;

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let data = bytes.get(offset..offset + 2).ok_or("잘린 ZIP 파일")?;
    Ok(u16::from_le_bytes([data[0], data[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let data = bytes.get(offset..offset + 4).ok_or("잘린 ZIP 파일")?;
    Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

pub fn read_zip(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    if bytes.len() < 22 {
        return Err("ZIP 파일이 너무 짧습니다".into());
    }
    let search_start = bytes.len().saturating_sub(65_557);
    let eocd = (search_start..=bytes.len() - 22)
        .rev()
        .find(|&i| bytes.get(i..i + 4) == Some(&b"PK\x05\x06"[..]))
        .ok_or("ZIP 끝 기록을 찾을 수 없습니다")?;
    let count = u16_at(bytes, eocd + 10)? as usize;
    let central_size = u32_at(bytes, eocd + 12)? as usize;
    let central_offset = u32_at(bytes, eocd + 16)? as usize;
    if count == 0xffff || central_size == u32::MAX as usize || central_offset == u32::MAX as usize {
        return Err("ZIP64 파일은 아직 지원하지 않습니다".into());
    }
    let central_end = central_offset
        .checked_add(central_size)
        .ok_or("잘못된 ZIP 크기")?;
    if central_end > bytes.len() || central_end > eocd {
        return Err("잘못된 ZIP 중앙 디렉터리".into());
    }
    let mut cursor = central_offset;
    let mut total = 0usize;
    let mut files = Vec::with_capacity(count.min(10_000));
    for _ in 0..count {
        if u32_at(bytes, cursor)? != 0x0201_4b50 {
            return Err("잘못된 ZIP 항목".into());
        }
        let flags = u16_at(bytes, cursor + 8)?;
        let method = u16_at(bytes, cursor + 10)?;
        let expected_crc = u32_at(bytes, cursor + 16)?;
        let compressed = u32_at(bytes, cursor + 20)? as usize;
        let uncompressed = u32_at(bytes, cursor + 24)? as usize;
        let name_len = u16_at(bytes, cursor + 28)? as usize;
        let extra_len = u16_at(bytes, cursor + 30)? as usize;
        let comment_len = u16_at(bytes, cursor + 32)? as usize;
        let local_offset = u32_at(bytes, cursor + 42)? as usize;
        let name_start = cursor.checked_add(46).ok_or("잘못된 ZIP 오프셋")?;
        let name_end = name_start.checked_add(name_len).ok_or("잘못된 ZIP 이름")?;
        let name_bytes = bytes.get(name_start..name_end).ok_or("잘린 ZIP 이름")?;
        let name = String::from_utf8_lossy(name_bytes).replace('\\', "/");
        cursor = name_end
            .checked_add(extra_len)
            .and_then(|v| v.checked_add(comment_len))
            .ok_or("잘못된 ZIP 항목 크기")?;
        if cursor > central_end {
            return Err("잘린 ZIP 중앙 디렉터리".into());
        }
        if name.ends_with('/') {
            continue;
        }
        if flags & 1 != 0 {
            return Err("암호로 보호된 ZIP은 지원하지 않습니다".into());
        }
        if uncompressed > MAX_FILE
            || compressed > MAX_FILE
            || total.saturating_add(uncompressed) > MAX_TOTAL
        {
            return Err(
                "ZIP의 미디어 용량이 안전 한도를 넘습니다 (파일 100 MB, 전체 250 MB)".into(),
            );
        }
        if u32_at(bytes, local_offset)? != 0x0403_4b50 {
            return Err("잘못된 ZIP 로컬 항목".into());
        }
        let local_name = u16_at(bytes, local_offset + 26)? as usize;
        let local_extra = u16_at(bytes, local_offset + 28)? as usize;
        let data_start = local_offset
            .checked_add(30)
            .and_then(|v| v.checked_add(local_name))
            .and_then(|v| v.checked_add(local_extra))
            .ok_or("잘못된 ZIP 데이터 오프셋")?;
        let data_end = data_start
            .checked_add(compressed)
            .ok_or("잘못된 ZIP 데이터 크기")?;
        let raw = bytes.get(data_start..data_end).ok_or("잘린 ZIP 데이터")?;
        let data = match method {
            0 => raw.to_vec(),
            8 => {
                let decoder = flate2::read::DeflateDecoder::new(raw);
                let mut output = Vec::with_capacity(uncompressed.min(MAX_FILE));
                decoder
                    .take((uncompressed + 1) as u64)
                    .read_to_end(&mut output)
                    .map_err(|e| e.to_string())?;
                output
            }
            _ => return Err(format!("지원하지 않는 ZIP 압축 방식: {method}")),
        };
        if data.len() != uncompressed || crc32fast::hash(&data) != expected_crc {
            return Err(format!("ZIP 항목 손상: {name}"));
        }
        total += data.len();
        files.push((name, data));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_truncated_archive() {
        assert!(read_zip(b"PK\x03\x04").is_err());
    }

    #[test]
    fn reads_deflated_day_one_export() {
        let files = read_zip(include_bytes!("../../tests/fixtures/dayone-text.zip")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "Sample.json");
        let value: serde_json::Value = serde_json::from_slice(&files[0].1).unwrap();
        assert_eq!(value["entries"][0]["text"], "A test day");
    }
}
