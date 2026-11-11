//! On-device text embeddings for Niki's diary search.
//!
//! The model (jhgan/ko-sroberta-multitask, int8 ONNX) runs in pure Rust through tract, so diaries never
//! leave the device for search and every release target (Windows, Apple Silicon, Intel Mac)
//! builds without native ONNX Runtime binaries. Vectors live in `vectors.bin` next to the
//! journal; they are derived data, rebuilt from the diary whenever an entry changes.
use crate::Entry;
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tract_onnx::prelude::*;

// A Korean sentence-embedding model. On a Korean diary benchmark it found the right entry for
// 12 of 12 meaning-only questions, against 8 for multilingual-e5-small and 11 for bge-m3 (5x
// larger and slower).
const REPO: &str = "jhgan/ko-sroberta-multitask";
const REVISION: &str = "8fca7c9c98c26599be0e14b9916b11a756a26f19";
/// Changing the model or chunking invalidates stored vectors.
const INDEX_VERSION: &str = "ko-sroberta-multitask@8fca7c9c/qint8/chunk150";
pub const DIM: usize = 768;
/// The model was trained on 128-token inputs, so passages stay short.
const MAX_TOKENS: usize = 128;
const CHUNK_CHARS: usize = 150;
const MAX_CHUNKS: usize = 16;
const MAGIC: &[u8; 8] = b"NIKIVEC1";

pub struct ModelFile {
    remote: &'static str,
    local: &'static str,
    sha256: &'static str,
    size: u64,
}

/// Pinned by revision, size and SHA-256 (the Hugging Face LFS object ids).
pub const FILES: [ModelFile; 2] = [
    ModelFile {
        remote: "onnx/model_qint8_avx512_vnni.onnx",
        local: "model.onnx",
        sha256: "ddd6107a06385baf8a76a1ccbfd8718c684a1a751708db116061069a1224ffd6",
        size: 111_326_851,
    },
    ModelFile {
        remote: "tokenizer.json",
        local: "tokenizer.json",
        sha256: "70f194d3bd8fc273ee0bca77b49404c6230ebaca2cfe0af04d6b82964e054660",
        size: 495_027,
    },
];

pub fn total_download_bytes() -> u64 {
    FILES.iter().map(|f| f.size).sum()
}

fn model_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("models").join("ko-sroberta-multitask")
}

fn index_path(data_dir: &Path) -> PathBuf {
    data_dir.join("vectors.bin")
}

pub fn installed(data_dir: &Path) -> bool {
    let dir = model_dir(data_dir);
    FILES
        .iter()
        .all(|f| fs::metadata(dir.join(f.local)).is_ok_and(|m| m.len() == f.size))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Downloads any missing model file, verifying size and SHA-256 before it is used.
pub fn download(data_dir: &Path, progress: &AtomicU64) -> Result<(), String> {
    let dir = model_dir(data_dir);
    fs::create_dir_all(&dir).map_err(|_| "모델 폴더를 만들 수 없습니다")?;
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(1_800))
        .build()
        .map_err(|_| "다운로드를 시작할 수 없습니다")?;
    let mut done = 0u64;
    progress.store(0, Ordering::SeqCst);
    for file in &FILES {
        let dest = dir.join(file.local);
        if fs::metadata(&dest).is_ok_and(|m| m.len() == file.size) {
            done += file.size;
            progress.store(done, Ordering::SeqCst);
            continue;
        }
        let url = format!("https://huggingface.co/{REPO}/resolve/{REVISION}/{}", file.remote);
        let mut response = client
            .get(url)
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|_| "검색 모델을 내려받지 못했습니다. 인터넷 연결을 확인해 주세요.")?;
        let part = dir.join(format!("{}.part", file.local));
        let mut out = fs::File::create(&part).map_err(|_| "모델 파일을 쓸 수 없습니다")?;
        let mut digest = ring::digest::Context::new(&ring::digest::SHA256);
        let mut buffer = vec![0u8; 1 << 16];
        let mut received = 0u64;
        let result = (|| -> Result<(), String> {
            loop {
                let n = response
                    .read(&mut buffer)
                    .map_err(|_| "다운로드가 중단되었습니다. 다시 시도해 주세요.")?;
                if n == 0 {
                    break;
                }
                received += n as u64;
                if received > file.size {
                    return Err("모델 파일 크기가 예상과 다릅니다".into());
                }
                digest.update(&buffer[..n]);
                out.write_all(&buffer[..n])
                    .map_err(|_| "모델 파일을 쓸 수 없습니다")?;
                progress.store(done + received, Ordering::SeqCst);
            }
            if received != file.size || hex(digest.finish().as_ref()) != file.sha256 {
                return Err("모델 파일 검증에 실패했습니다. 다시 시도해 주세요.".into());
            }
            out.sync_all().map_err(|_| "모델 파일을 쓸 수 없습니다")?;
            Ok(())
        })();
        drop(out);
        if let Err(e) = result {
            let _ = fs::remove_file(&part);
            return Err(e);
        }
        fs::rename(&part, &dest).map_err(|_| "모델 파일을 저장할 수 없습니다")?;
        done += file.size;
        progress.store(done, Ordering::SeqCst);
    }
    Ok(())
}

pub struct Embedder {
    tokenizer: tokenizers::Tokenizer,
    model: Arc<TypedRunnableModel>,
    /// Which model input each position is: 0 ids, 1 attention mask, 2 token types.
    inputs: Vec<u8>,
}

impl Embedder {
    pub fn load(data_dir: &Path) -> Result<Self, String> {
        if !installed(data_dir) {
            return Err("검색 모델이 아직 없습니다".into());
        }
        let dir = model_dir(data_dir);
        let fail = |_| "검색 모델을 불러오지 못했습니다".to_string();
        let tokenizer = tokenizers::Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|_| "검색 모델을 불러오지 못했습니다".to_string())?;
        let mut model = tract_onnx::onnx()
            .model_for_path(dir.join("model.onnx"))
            .map_err(fail)?;
        let mut inputs = Vec::new();
        for outlet in model.input_outlets().map_err(fail)?.to_vec() {
            let name = model.node(outlet.node).name.as_str();
            inputs.push(match name {
                "input_ids" => 0,
                "attention_mask" => 1,
                "token_type_ids" => 2,
                _ => return Err("검색 모델 형식이 올바르지 않습니다".into()),
            });
        }
        let batch = model.symbols.sym("batch_size");
        let length = model.symbols.sym("sequence_length");
        for i in 0..inputs.len() {
            model = model
                .with_input_fact(i, i64::fact([batch.to_dim(), length.to_dim()]).into())
                .map_err(fail)?;
        }
        let model = model
            .into_optimized()
            .and_then(|m| m.into_runnable())
            .map_err(fail)?;
        Ok(Self { tokenizer, model, inputs })
    }

    /// L2-normalized mean-pooled embedding (the model's sentence-transformers pooling).
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|_| "문장을 처리하지 못했습니다".to_string())?;
        let mut ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        if ids.len() > MAX_TOKENS {
            let end = *ids.last().unwrap_or(&2);
            ids.truncate(MAX_TOKENS - 1);
            ids.push(end);
        }
        let n = ids.len();
        let fail = |_| "임베딩 계산에 실패했습니다".to_string();
        let mut values: TVec<TValue> = tvec!();
        for kind in &self.inputs {
            let data = match kind {
                0 => ids.clone(),
                1 => vec![1i64; n],
                _ => vec![0i64; n],
            };
            values.push(Tensor::from_shape(&[1, n], &data).map_err(fail)?.into());
        }
        let outputs = self.model.run(values).map_err(fail)?;
        let hidden = outputs[0].to_plain_array_view::<f32>().map_err(fail)?;
        if hidden.ndim() != 3 || hidden.shape()[2] != DIM {
            return Err("검색 모델 출력 형식이 올바르지 않습니다".into());
        }
        let rows = hidden.shape()[1];
        let mut vector = vec![0f32; DIM];
        for i in 0..rows {
            for (j, v) in vector.iter_mut().enumerate() {
                *v += hidden[[0, i, j]];
            }
        }
        let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
        Ok(vector.iter().map(|x| x / norm).collect())
    }
}

/// Splits an entry into ~150-character passages at whitespace, title first.
pub fn chunks(entry: &Entry) -> Vec<String> {
    let text = format!("{}\n{}", entry.title.trim(), entry.body.trim());
    let mut out = Vec::new();
    let mut current = String::new();
    let mut count = 0usize;
    for c in text.chars() {
        current.push(c);
        count += 1;
        if count >= CHUNK_CHARS && (c.is_whitespace() || count >= CHUNK_CHARS * 3 / 2) {
            out.push(std::mem::take(&mut current));
            count = 0;
            if out.len() == MAX_CHUNKS {
                return out;
            }
        }
    }
    if !current.trim().is_empty() {
        out.push(current);
    }
    out.retain(|chunk| !chunk.trim().is_empty());
    out
}

pub struct Indexed {
    pub updated_at: String,
    pub vectors: Vec<Vec<f32>>,
}

#[derive(Default)]
pub struct VectorIndex {
    pub entries: HashMap<String, Indexed>,
}

impl VectorIndex {
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|bytes| Self::decode(&bytes))
            .unwrap_or_default()
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        struct Reader<'a>(&'a [u8]);
        impl<'a> Reader<'a> {
            fn take(&mut self, n: usize) -> Option<&'a [u8]> {
                if self.0.len() < n {
                    return None;
                }
                let (head, rest) = self.0.split_at(n);
                self.0 = rest;
                Some(head)
            }
            fn len(&mut self) -> Option<usize> {
                Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?) as usize)
            }
            fn string(&mut self) -> Option<String> {
                let n = self.len()?;
                String::from_utf8(self.take(n)?.to_vec()).ok()
            }
        }
        let mut r = Reader(bytes);
        if r.take(8)? != MAGIC || r.string()? != INDEX_VERSION {
            return None;
        }
        let count = r.len()?;
        let mut entries = HashMap::new();
        for _ in 0..count {
            let id = r.string()?;
            let updated_at = r.string()?;
            let n = r.len()?;
            if n > MAX_CHUNKS {
                return None;
            }
            let mut vectors = Vec::with_capacity(n);
            for _ in 0..n {
                vectors.push(
                    r.take(DIM * 4)?
                        .chunks_exact(4)
                        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                        .collect(),
                );
            }
            entries.insert(id, Indexed { updated_at, vectors });
        }
        r.0.is_empty().then_some(Self { entries })
    }

    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + self.entries.len() * (64 + 2 * DIM * 4));
        let put_str = |out: &mut Vec<u8>, s: &str| {
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        };
        out.extend_from_slice(MAGIC);
        put_str(&mut out, INDEX_VERSION);
        out.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for (id, item) in &self.entries {
            put_str(&mut out, id);
            put_str(&mut out, &item.updated_at);
            out.extend_from_slice(&(item.vectors.len() as u32).to_le_bytes());
            for vector in &item.vectors {
                for x in vector {
                    out.extend_from_slice(&x.to_le_bytes());
                }
            }
        }
        out
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let temp = path.with_extension("bin.tmp");
        fs::write(&temp, self.encode()).map_err(|_| "검색 색인을 저장할 수 없습니다")?;
        fs::rename(&temp, path).map_err(|_| "검색 색인을 저장할 수 없습니다".into())
    }

    /// Live entries with no vectors or vectors for an older version of the text, newest first.
    pub fn stale<'a>(&self, entries: &'a [Entry]) -> Vec<&'a Entry> {
        let mut stale: Vec<&Entry> = crate::memory::active(entries)
            .into_iter()
            .filter(|e| {
                self.entries
                    .get(&e.id)
                    .is_none_or(|item| item.updated_at != e.updated_at)
            })
            .collect();
        stale.sort_by(|a, b| b.date.cmp(&a.date));
        stale
    }

    /// Drops vectors of deleted or emptied entries. Returns whether anything changed.
    pub fn prune(&mut self, entries: &[Entry]) -> bool {
        let live: std::collections::HashSet<&str> = crate::memory::active(entries)
            .into_iter()
            .map(|e| e.id.as_str())
            .collect();
        let before = self.entries.len();
        self.entries.retain(|id, _| live.contains(id.as_str()));
        before != self.entries.len()
    }

    /// Entry ids by best passage similarity to `query`, highest first.
    pub fn scores(&self, query: &[f32]) -> Vec<(String, f32)> {
        let mut scored: Vec<(String, f32)> = self
            .entries
            .iter()
            .filter_map(|(id, item)| {
                item.vectors
                    .iter()
                    .map(|v| v.iter().zip(query).map(|(a, b)| a * b).sum::<f32>())
                    .reduce(f32::max)
                    .map(|best| (id.clone(), best))
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }
}

#[derive(Default)]
struct Loaded {
    embedder: Option<Embedder>,
    index: Option<VectorIndex>,
}

/// After a failed download, wait this long before trying again.
const RETRY_AFTER: Duration = Duration::from_secs(60);

fn should_retry(last_attempt: Option<std::time::Instant>) -> bool {
    last_attempt.is_none_or(|at| at.elapsed() >= RETRY_AFTER)
}

/// Shared embedding state: the loaded model, the vector index and download progress.
#[derive(Clone, Default)]
pub struct Vectors {
    loaded: Arc<Mutex<Loaded>>,
    pub downloading: Arc<AtomicBool>,
    pub progress: Arc<AtomicU64>,
    pub failed: Arc<AtomicBool>,
    last_attempt: Arc<Mutex<Option<std::time::Instant>>>,
}

impl Vectors {
    /// Fetches the model in the background when it is missing, once a GPT account is connected
    /// (checked whenever the app reads the Codex status), retrying at most once a minute after a
    /// failure. The model is kept across app updates, so this normally happens once.
    pub fn ensure_model(&self, data_dir: &Path) {
        if installed(data_dir) {
            return;
        }
        let Ok(mut last) = self.last_attempt.lock() else {
            return;
        };
        if !should_retry(*last) || self.downloading.swap(true, Ordering::SeqCst) {
            return;
        }
        *last = Some(std::time::Instant::now());
        drop(last);
        let (vectors, dir) = (self.clone(), data_dir.to_path_buf());
        std::thread::spawn(move || {
            let ok = download(&dir, &vectors.progress).is_ok();
            vectors.failed.store(!ok, Ordering::SeqCst);
            vectors.downloading.store(false, Ordering::SeqCst);
        });
    }

    fn with<T>(
        &self,
        data_dir: &Path,
        f: impl FnOnce(&Embedder, &mut VectorIndex) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut loaded = self.loaded.lock().map_err(|_| "검색 색인을 사용할 수 없습니다")?;
        if loaded.embedder.is_none() {
            loaded.embedder = Some(Embedder::load(data_dir)?);
        }
        if loaded.index.is_none() {
            loaded.index = Some(VectorIndex::load(&index_path(data_dir)));
        }
        let Loaded { embedder, index } = &mut *loaded;
        match (embedder.as_ref(), index.as_mut()) {
            (Some(embedder), Some(index)) => f(embedder, index),
            _ => Err("검색 색인을 사용할 수 없습니다".into()),
        }
    }

    /// Entries still waiting for vectors (0 when the model is not installed).
    pub fn pending(&self, data_dir: &Path, entries: &[Entry]) -> usize {
        if !installed(data_dir) {
            return 0;
        }
        let Ok(mut loaded) = self.loaded.lock() else {
            return 0;
        };
        loaded
            .index
            .get_or_insert_with(|| VectorIndex::load(&index_path(data_dir)))
            .stale(entries)
            .len()
    }

    /// Embeds stale entries (newest first) for about `budget`, at least one, and saves the
    /// index. Returns how many entries are still waiting.
    pub fn index_stale(&self, data_dir: &Path, entries: &[Entry], budget: Duration) -> Result<usize, String> {
        let started = std::time::Instant::now();
        self.with(data_dir, |embedder, index| {
            let mut changed = index.prune(entries);
            let stale = index.stale(entries);
            let (mut remaining, mut done) = (stale.len(), 0usize);
            for entry in stale {
                if done > 0 && started.elapsed() >= budget {
                    break;
                }
                let vectors = chunks(entry)
                    .iter()
                    .map(|chunk| embedder.embed(chunk))
                    .collect::<Result<Vec<_>, _>>()?;
                index.entries.insert(
                    entry.id.clone(),
                    Indexed { updated_at: entry.updated_at.clone(), vectors },
                );
                changed = true;
                remaining -= 1;
                done += 1;
            }
            if changed {
                index.save(&index_path(data_dir))?;
            }
            Ok(remaining)
        })
    }

    /// Similarity of every indexed entry to `query`, after indexing a few recent changes inline.
    pub fn search(&self, data_dir: &Path, entries: &[Entry], query: &str) -> Result<Vec<(String, f32)>, String> {
        self.index_stale(data_dir, entries, Duration::from_secs(2))?;
        self.with(data_dir, |embedder, index| {
            let vector = embedder.embed(query)?;
            Ok(index.scores(&vector))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, body: &str) -> Entry {
        serde_json::from_value(serde_json::json!({
            "id": id, "journal_id": "j", "title": "제목", "body": body, "date": "2026-01-01",
            "created_at": "v", "updated_at": "v1", "favorite": false, "tags": [], "location": "",
            "weather": "", "attachments": [], "deleted_at": null, "external_id": null
        }))
        .unwrap()
    }

    #[test]
    fn chunks_split_long_entries_and_cap_count() {
        assert_eq!(chunks(&entry("a", "짧은 일기")), vec!["제목\n짧은 일기".to_owned()]);
        let long = chunks(&entry("b", &"가나다라 ".repeat(150)));
        assert!(long.len() > 1 && long.len() <= MAX_CHUNKS);
        assert!(long.iter().all(|c| c.chars().count() <= CHUNK_CHARS * 3 / 2 + 1));
        assert_eq!(chunks(&entry("c", &"가".repeat(10_000))).len(), MAX_CHUNKS);
    }

    #[test]
    fn failed_downloads_retry_at_most_once_a_minute() {
        assert!(should_retry(None));
        assert!(!should_retry(Some(std::time::Instant::now())));
        let long_ago = std::time::Instant::now().checked_sub(RETRY_AFTER + Duration::from_secs(1));
        if let Some(at) = long_ago {
            assert!(should_retry(Some(at)));
        }
    }

    #[test]
    fn index_round_trips_and_tracks_staleness() {
        let dir = std::env::temp_dir().join(format!("niki-vec-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("vectors.bin");
        let mut list = vec![entry("a", "하나"), entry("b", "둘")];
        let mut index = VectorIndex::default();
        assert_eq!(index.stale(&list).len(), 2);
        let mut v = vec![0f32; DIM];
        v[0] = 1.0;
        index.entries.insert("a".into(), Indexed { updated_at: "v1".into(), vectors: vec![v.clone()] });
        index.save(&path).unwrap();
        let restored = VectorIndex::load(&path);
        assert_eq!(restored.entries["a"].vectors[0], v);
        assert_eq!(restored.stale(&list).len(), 1);
        assert_eq!(restored.scores(&v)[0], ("a".to_owned(), 1.0));
        list[0].updated_at = "v2".into();
        assert_eq!(restored.stale(&list).len(), 2, "edited entries are re-embedded");
        let mut pruned = restored;
        list.remove(0);
        assert!(pruned.prune(&list));
        assert!(pruned.entries.is_empty());
        fs::write(&path, b"garbage").unwrap();
        assert!(VectorIndex::load(&path).entries.is_empty(), "corrupt index is rebuilt");
        fs::remove_dir_all(dir).unwrap();
    }

    /// Needs the model files: NIKI_EMBED_DIR=<dir with model.onnx and tokenizer.json>.
    #[test]
    #[ignore = "requires the downloaded embedding model"]
    fn live_model_ranks_korean_diary_by_meaning() {
        let data = std::env::temp_dir().join(format!("niki-e5-{}", uuid::Uuid::new_v4()));
        let dir = model_dir(&data);
        fs::create_dir_all(&dir).unwrap();
        let source = PathBuf::from(std::env::var_os("NIKI_EMBED_DIR").expect("NIKI_EMBED_DIR"));
        for f in &FILES {
            fs::copy(source.join(f.local), dir.join(f.local)).unwrap();
        }
        let embedder = Embedder::load(&data).unwrap();
        // Each question shares no words with its entry, so only meaning can match them.
        let docs = [
            "혼자 집에 있으니 너무 쓸쓸하고 아무도 연락이 없었다.",
            "친구들과 제주도 바닷가에서 수영하고 회를 먹었다. 정말 즐거웠다.",
            "팀장님께 보고서 때문에 크게 혼났다.",
            "엄마랑 저녁 메뉴 때문에 말다툼을 했다. 서운했다.",
            "헬스장에서 하체를 하고 러닝머신을 30분 뛰었다.",
            "새벽 세 시까지 잠이 오지 않아 뒤척였다.",
            "강아지 보리가 아파서 동물병원에 다녀왔다.",
            "드디어 자격증 시험에 합격했다! 너무 기쁘다.",
            "월세가 또 올라서 통장 잔고가 걱정이다.",
            "오랜만에 할머니 댁에 가서 김장을 도왔다.",
            "소개팅에서 만난 사람과 두 번째 데이트를 했다. 설렌다.",
            "회의 발표를 망쳐서 하루 종일 자책했다.",
        ];
        let queries = [
            "외로웠던 때", "여름 휴가 여행", "직장에서 꾸중 들은 날", "가족과 갈등", "운동한 기록",
            "불면증", "반려동물 건강", "시험 결과가 좋았던 날", "돈 문제로 불안", "명절 같은 집안 행사",
            "연애 시작", "실수해서 스스로를 탓함",
        ];
        let tokens = embedder.tokenizer.encode("가나다라마바사 ".repeat(19).as_str(), true).unwrap().len();
        eprintln!("tokens for ~150 Korean chars: {tokens}");
        let doc_vectors: Vec<Vec<f32>> = docs.iter().map(|d| embedder.embed(d).unwrap()).collect();
        for (expected, q) in queries.iter().enumerate() {
            let qv = embedder.embed(q).unwrap();
            let best = doc_vectors
                .iter()
                .enumerate()
                .map(|(i, d)| (i, d.iter().zip(&qv).map(|(a, b)| a * b).sum::<f32>()))
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .unwrap()
                .0;
            assert_eq!(best, expected, "{q}");
        }
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    #[ignore = "requires the downloaded embedding model"]
    fn live_index_search_and_reindex_after_edit() {
        let data = std::env::temp_dir().join(format!("niki-e5-{}", uuid::Uuid::new_v4()));
        let dir = model_dir(&data);
        fs::create_dir_all(&dir).unwrap();
        let source = PathBuf::from(std::env::var_os("NIKI_EMBED_DIR").expect("NIKI_EMBED_DIR"));
        for f in &FILES {
            fs::copy(source.join(f.local), dir.join(f.local)).unwrap();
        }
        let mut list = vec![
            entry("lonely", "혼자 집에 있으니 너무 쓸쓸하고 아무도 연락이 없었다."),
            entry("trip", "친구들과 제주도 바닷가에서 수영했다. 정말 행복했다."),
            entry("work", "팀장님께 보고서 때문에 크게 혼났다."),
        ];
        let vectors = Vectors::default();
        assert_eq!(vectors.pending(&data, &list), 3);
        assert_eq!(vectors.index_stale(&data, &list, Duration::ZERO).unwrap(), 2, "a zero budget still makes progress");
        let scores = vectors.search(&data, &list, "외로웠던 때").unwrap();
        assert_eq!(scores.len(), 3, "search indexes what is left first");
        assert_eq!(scores[0].0, "lonely");
        // Editing re-embeds only that entry, and the change is what gets found.
        list[2].body = "가족과 바다로 여름 휴가를 떠났다.".into();
        list[2].updated_at = "v2".into();
        assert_eq!(vectors.pending(&data, &list), 1);
        let scores = Vectors::default().search(&data, &list, "여름 휴가").unwrap();
        assert!(scores[0].0 == "work" || scores[0].0 == "trip", "{scores:?}");
        assert_eq!(VectorIndex::load(&index_path(&data)).entries["work"].updated_at, "v2");
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    #[ignore = "downloads about 112 MB from Hugging Face"]
    fn live_download_is_pinned_and_verified() {
        let data = std::env::temp_dir().join(format!("niki-dl-{}", uuid::Uuid::new_v4()));
        let progress = AtomicU64::new(0);
        assert!(!installed(&data));
        download(&data, &progress).unwrap();
        assert!(installed(&data));
        assert_eq!(progress.load(Ordering::SeqCst), total_download_bytes());
        Embedder::load(&data).unwrap();
        // A tampered file is caught by the size check and fetched again.
        fs::write(model_dir(&data).join("tokenizer.json"), b"{}").unwrap();
        assert!(!installed(&data));
        download(&data, &progress).unwrap();
        assert!(installed(&data));
        fs::remove_dir_all(data).unwrap();
    }
}
