#[path = "installer/mod.rs"]
mod app_installer;
#[path = "updater/mod.rs"]
mod app_updater;
#[path = "drive/auth.rs"]
mod drive_auth;
#[path = "drive/transport.rs"]
mod drive_transport;
#[path = "sync/crypto.rs"]
mod sync_crypto;
#[path = "sync/engine.rs"]
mod sync_engine;
#[path = "sync/media.rs"]
mod sync_media;
#[path = "sync/merge.rs"]
mod sync_merge;
#[path = "archive/zip_read.rs"]
mod zip_read;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{Manager, State};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Journal {
    pub id: String,
    pub name: String,
    pub color: String,
    pub created_at: String,
}
impl Default for Journal {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "새 일지".into(),
            color: "#576b61".into(),
            created_at: now(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub id: String,
    pub journal_id: String,
    pub title: String,
    pub body: String,
    pub date: String,
    pub created_at: String,
    pub updated_at: String,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub location: String,
    pub weather: String,
    pub attachments: Vec<Attachment>,
    pub deleted_at: Option<String>,
    pub external_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SyncConflict {
    id: String,
    entry_id: String,
    local: Option<Entry>,
    remote: Option<Entry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PendingResolution {
    conflict: SyncConflict,
    choice: String,
    chosen: Option<Entry>,
}
impl Default for Entry {
    fn default() -> Self {
        let timestamp = now();
        Self {
            id: Uuid::new_v4().to_string(),
            journal_id: String::new(),
            title: String::new(),
            body: String::new(),
            date: timestamp[..10].into(),
            created_at: timestamp.clone(),
            updated_at: timestamp,
            favorite: false,
            tags: Vec::new(),
            location: String::new(),
            weather: String::new(),
            attachments: Vec::new(),
            deleted_at: None,
            external_id: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub body: String,
}
impl Default for Template {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::new(),
            body: String::new(),
        }
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default)]
struct Store {
    version: u32,
    nickname: String,
    updater_config: app_updater::UpdaterConfig,
    journals: Vec<Journal>,
    entries: Vec<Entry>,
    templates: Vec<Template>,
    password_hash: Option<String>,
    sync_conflicts: Vec<SyncConflict>,
    pending_resolutions: Vec<PendingResolution>,
    device_id: String,
    sync_cursor: Option<String>,
    last_sync_at: Option<String>,
    last_sync_error: Option<String>,
    sync_paused: bool,
    sync_base_entries: HashMap<String, Entry>,
    sync_base_journals: HashMap<String, Journal>,
    sync_base_templates: HashMap<String, Template>,
    sync_base_nickname: Option<String>,
    media_manifests: HashMap<String, sync_media::Manifest>,
    seen_remote_objects: HashMap<String, String>,
    remote_heads: HashMap<String, Vec<String>>,
    revision_parents: HashMap<String, Vec<String>>,
}

#[derive(Clone, Default, Serialize)]
struct SyncStatus {
    connected: bool,
    has_oauth_client: bool,
    client_id: Option<String>,
    account_email: Option<String>,
    account_name: Option<String>,
    has_sync_key: bool,
    sync_paused: bool,
    last_sync_at: Option<String>,
    last_sync_error: Option<String>,
    pending_upload_count: usize,
    conflict_count: usize,
    pending_resolution_count: usize,
    remote_object_count: usize,
    synced_media_bytes: usize,
}

#[derive(Serialize)]
struct Snapshot {
    locked: bool,
    lock_enabled: bool,
    app_version: String,
    installed_on_pc: bool,
    running_installed: bool,
    install_path: String,
    updater_config: app_updater::UpdaterConfig,
    nickname: String,
    journals: Vec<Journal>,
    entries: Vec<Entry>,
    templates: Vec<Template>,
    sync_conflicts: Vec<SyncConflict>,
    pending_resolution_count: usize,
    sync_status: SyncStatus,
    synced_entry_ids: Vec<String>,
}

struct AppState {
    data_dir: PathBuf,
    store: Mutex<Store>,
    unlocked: AtomicBool,
    syncing: AtomicBool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct DriveConfig {
    client_id: String,
    account_email: Option<String>,
    account_name: Option<String>,
    sync_key: Option<String>,
}

const DEFAULT_GOOGLE_CLIENT_ID: Option<&str> = option_env!("NIKI_GOOGLE_CLIENT_ID");
const DEFAULT_GOOGLE_CLIENT_SECRET: Option<&str> = option_env!("NIKI_GOOGLE_CLIENT_SECRET");

fn load_drive_config(data_dir: &std::path::Path) -> Option<DriveConfig> {
    let path = data_dir.join("drive-config.json");
    if let Ok(bytes) = fs::read(path)
        && let Ok(config) = serde_json::from_slice::<DriveConfig>(&bytes)
        && drive_auth::validate_client_id(&config.client_id).is_ok()
        && config.client_id != "niki-desktop.apps.googleusercontent.com"
    {
        return Some(config);
    }
    let mut candidates = vec![
        data_dir.join("oauth-client.json"),
        PathBuf::from("oauth-client.json"),
    ];
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join("oauth-client.json"));
        if let Some(contents) = dir.parent() {
            candidates.push(contents.join("Resources").join("oauth-client.json"));
        }
        if let Some(root) = dir.parent().and_then(|p| p.parent()) {
            candidates.push(root.join("oauth-client.json"));
        }
    }
    for candidate in candidates {
        if let Ok(bytes) = fs::read(candidate)
            && let Ok((client_id, client_secret)) = drive_auth::parse_credentials_json(&bytes)
        {
            if !client_secret.is_empty() {
                let _ = drive_auth::save_client_secret(&client_id, &client_secret);
            }
            return Some(DriveConfig {
                client_id,
                ..DriveConfig::default()
            });
        }
    }
    if let Ok(env_id) = std::env::var("NIKI_GOOGLE_CLIENT_ID") {
        let client_id = env_id.trim().to_string();
        if drive_auth::validate_client_id(&client_id).is_ok() {
            return Some(DriveConfig {
                client_id,
                ..DriveConfig::default()
            });
        }
    }
    if let Some(built_in_id) = DEFAULT_GOOGLE_CLIENT_ID {
        let client_id = built_in_id.trim().to_string();
        if drive_auth::validate_client_id(&client_id).is_ok() {
            return Some(DriveConfig {
                client_id,
                ..DriveConfig::default()
            });
        }
    }
    None
}

fn save_drive_config(data_dir: &std::path::Path, config: &DriveConfig) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(config).map_err(err)?;
    let temp = data_dir.join("drive-config.json.tmp");
    fs::write(&temp, bytes).map_err(err)?;
    fs::rename(&temp, data_dir.join("drive-config.json")).map_err(err)
}

fn load_local_sync_key(data_dir: &std::path::Path) -> Option<[u8; 32]> {
    if let Ok(key) = drive_auth::load_sync_key() {
        return Some(key);
    }
    let cfg = load_drive_config(data_dir)?;
    let encoded = cfg.sync_key.as_deref()?;
    sync_crypto::decode_recovery_key(encoded).ok()
}

fn persist_local_sync_key(data_dir: &std::path::Path, key: &[u8; 32]) {
    let _ = drive_auth::save_sync_key(key);
    if let Some(mut cfg) = load_drive_config(data_dir) {
        cfg.sync_key = Some(sync_crypto::encode_recovery_key(key));
        let _ = save_drive_config(data_dir, &cfg);
    }
}

fn connected_client_id(data_dir: &std::path::Path) -> Option<String> {
    let config = load_drive_config(data_dir)?;
    if drive_auth::load_refresh_token(&config.client_id).is_ok() {
        Some(config.client_id)
    } else {
        None
    }
}

#[tauri::command]
async fn drive_connect(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    client_id: String,
    client_secret: String,
) -> Result<Option<String>, String> {
    ensure_unlocked(&state)?;
    let saved_config = load_drive_config(&state.data_dir);
    let mut client_id = client_id.trim().to_string();
    if client_id.is_empty()
        && let Some(cfg) = &saved_config
    {
        client_id = cfg.client_id.trim().to_string();
    }
    drive_auth::validate_client_id(&client_id)?;
    let mut client_secret = client_secret.trim().to_string();
    if client_secret.is_empty()
        && let Some(saved_secret) = drive_auth::load_client_secret(&client_id)
    {
        client_secret = saved_secret;
    }
    if client_secret.is_empty()
        && let Some(built_in_secret) = DEFAULT_GOOGLE_CLIENT_SECRET
    {
        client_secret = built_in_secret.trim().to_string();
    }
    if client_secret.len() > 512 {
        return Err("클라이언트 비밀번호가 너무 깁니다".into());
    }
    let data_dir = state.data_dir.clone();
    let existing_key = load_local_sync_key(&data_dir);
    let (generated_key, reset_needed) = tauri::async_runtime::spawn_blocking(move || {
        let secret = if client_secret.is_empty() {
            None
        } else {
            Some(client_secret.as_str())
        };
        let tokens = drive_auth::authorize(&client_id, secret)?;
        if tokens.access_token.is_empty() {
            return Err("Google 액세스 토큰을 받지 못했습니다".into());
        }
        let refresh = tokens
            .refresh_token
            .ok_or("Google에서 새로 고침 토큰을 받지 못했습니다")?;
        drive_auth::save_refresh_token(&client_id, &refresh)?;
        if !client_secret.is_empty() {
            drive_auth::save_client_secret(&client_id, &client_secret)?;
        }
        let profile = drive_auth::fetch_user_profile(&tokens.access_token);
        let (account_name, account_email) = match profile {
            Some((name, email)) => (
                (!name.is_empty()).then_some(name),
                (!email.is_empty()).then_some(email),
            ),
            None => (None, None),
        };
        let active_key = if let Some(ref email) = account_email {
            drive_transport::derive_account_sync_key(email)
        } else {
            existing_key.unwrap_or(drive_transport::ACCOUNT_ENVELOPE_KEY)
        };
        let mut reset_needed = existing_key != Some(active_key);
        if let Ok(client) = drive_transport::DriveClient::new() {
            if let Ok(cleaned) = client.ensure_cloud_sync_key(&tokens.access_token, active_key) {
                if cleaned {
                    reset_needed = true;
                }
            }
        }
        let _ = drive_auth::save_sync_key(&active_key);
        let encoded_key = sync_crypto::encode_recovery_key(&active_key);
        save_drive_config(
            &data_dir,
            &DriveConfig {
                client_id,
                account_email,
                account_name,
                sync_key: Some(encoded_key),
            },
        )?;
        Ok::<(Option<String>, bool), String>((None, reset_needed))
    })
    .await
    .map_err(err)??;
    if reset_needed {
        let _ = mutate(&state, |store| {
            store.sync_cursor = None;
            store.seen_remote_objects.clear();
            store.remote_heads.clear();
            store.revision_parents.clear();
            store.sync_base_entries.clear();
            store.sync_base_journals.clear();
            store.sync_base_templates.clear();
            store.sync_base_nickname = None;
            store.media_manifests.clear();
            store.last_sync_error = None;
            Ok(())
        });
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    Ok(generated_key)
}

#[tauri::command]
fn drive_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    ensure_unlocked(&state)?;
    let Some(mut config) = load_drive_config(&state.data_dir) else {
        return Ok(());
    };
    let _ = drive_auth::delete_refresh_token(&config.client_id);
    config.account_email = None;
    config.account_name = None;
    save_drive_config(&state.data_dir, &config)
}

#[tauri::command]
fn drive_generate_sync_key(state: State<'_, AppState>) -> Result<String, String> {
    ensure_unlocked(&state)?;
    let key = sync_crypto::generate_key().map_err(err)?;
    drive_auth::save_sync_key(&key)?;
    mutate(&state, |store| {
        store.sync_cursor = None;
        store.seen_remote_objects.clear();
        store.remote_heads.clear();
        store.revision_parents.clear();
        store.sync_base_entries.clear();
        store.sync_base_journals.clear();
        store.sync_base_templates.clear();
        store.sync_base_nickname = None;
        store.media_manifests.clear();
        store.last_sync_error = None;
        Ok(())
    })?;
    Ok(sync_crypto::encode_recovery_key(&key))
}

#[tauri::command]
fn drive_get_sync_key(state: State<'_, AppState>) -> Result<Option<String>, String> {
    ensure_unlocked(&state)?;
    Ok(drive_auth::load_sync_key()
        .ok()
        .map(|k| sync_crypto::encode_recovery_key(&k)))
}

#[tauri::command]
fn drive_set_sync_key(state: State<'_, AppState>, recovery_key: String) -> Result<(), String> {
    ensure_unlocked(&state)?;
    let key = sync_crypto::decode_recovery_key(&recovery_key)?;
    drive_auth::save_sync_key(&key)?;
    mutate(&state, |store| {
        store.sync_cursor = None;
        store.seen_remote_objects.clear();
        store.remote_heads.clear();
        store.revision_parents.clear();
        store.sync_base_entries.clear();
        store.sync_base_journals.clear();
        store.sync_base_templates.clear();
        store.sync_base_nickname = None;
        store.media_manifests.clear();
        store.last_sync_error = None;
        Ok(())
    })
}

#[tauri::command]
fn drive_set_paused(state: State<'_, AppState>, paused: bool) -> Result<(), String> {
    mutate(&state, |store| {
        store.sync_paused = paused;
        Ok(())
    })
}

#[tauri::command]
async fn drive_sync_now(state: State<'_, AppState>) -> Result<sync_engine::SyncSummary, String> {
    ensure_unlocked(&state)?;
    if state.syncing.swap(true, Ordering::SeqCst) {
        return Err("이미 동기화가 진행 중입니다".into());
    }
    struct SyncGuard<'a>(&'a AtomicBool);
    impl Drop for SyncGuard<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let _guard = SyncGuard(&state.syncing);

    let (mut working_store, data_dir) = {
        let store = state.store.lock().map_err(err)?;
        if store.sync_paused {
            return Err("동기화가 일시 중지되어 있습니다. 먼저 동기화를 재개하세요".into());
        }
        (store.clone(), state.data_dir.clone())
    };

    let client_id = connected_client_id(&data_dir)
        .ok_or_else(|| "Google 계정을 먼저 연결하세요".to_string())?;
    let local_key = load_local_sync_key(&data_dir);

    let data_dir_for_task = data_dir.clone();
    let sync_result = tauri::async_runtime::spawn_blocking(move || {
        let token = drive_auth::acquire_access_token(&client_id)?;
        let mut cfg = load_drive_config(&data_dir_for_task);
        let mut account_email = cfg.as_ref().and_then(|c| c.account_email.clone());
        if account_email.is_none() {
            if let Some((name, email)) = drive_auth::fetch_user_profile(&token) {
                if let Some(ref mut c) = cfg {
                    if !name.is_empty() {
                        c.account_name = Some(name);
                    }
                    if !email.is_empty() {
                        c.account_email = Some(email.clone());
                    }
                    let _ = save_drive_config(&data_dir_for_task, c);
                }
                if !email.is_empty() {
                    account_email = Some(email);
                }
            }
        }

        let key = if let Some(ref email) = account_email {
            drive_transport::derive_account_sync_key(email)
        } else {
            local_key.unwrap_or(drive_transport::ACCOUNT_ENVELOPE_KEY)
        };

        let mut reset_needed = local_key != Some(key);
        let client = drive_transport::DriveClient::new()?;
        if let Ok(cleaned) = client.ensure_cloud_sync_key(&token, key) {
            if cleaned {
                reset_needed = true;
            }
        }
        persist_local_sync_key(&data_dir_for_task, &key);
        if reset_needed {
            working_store.sync_cursor = None;
            working_store.seen_remote_objects.clear();
            working_store.remote_heads.clear();
            working_store.revision_parents.clear();
            working_store.sync_base_entries.clear();
            working_store.sync_base_journals.clear();
            working_store.sync_base_templates.clear();
            working_store.sync_base_nickname = None;
            working_store.media_manifests.clear();
            working_store.last_sync_error = None;
        }
        let blobs = sync_media::DriveBlobs {
            client: &client,
            token: &token,
            key: &key,
        };
        let summary = sync_engine::sync_with_transport(
            &mut working_store,
            &data_dir_for_task,
            &blobs,
            |checkpoint| {
                let json = serde_json::to_vec_pretty(checkpoint).map_err(err)?;
                let dest = data_dir_for_task.join("journal.json");
                let temp = data_dir_for_task.join("journal.json.tmp");
                fs::write(&temp, json).map_err(err)?;
                fs::rename(&temp, &dest).map_err(err)?;
                Ok(())
            },
        )?;
        Ok::<(Store, sync_engine::SyncSummary), String>((working_store, summary))
    })
    .await
    .map_err(err)?;

    match sync_result {
        Ok((updated_store, summary)) => {
            let mut guard = state.store.lock().map_err(err)?;
            save(&state, &updated_store)?;
            *guard = updated_store;
            Ok(summary)
        }
        Err(error) => {
            let _ = mutate(&state, |store| {
                store.last_sync_error = Some(error.clone());
                Ok(())
            });
            Err(error)
        }
    }
}

fn now() -> String {
    Utc::now().to_rfc3339()
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn valid_date(value: &str) -> bool {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
}
fn valid_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}
fn data_path(state: &AppState) -> PathBuf {
    state.data_dir.join("journal.json")
}
fn media_path(state: &AppState, id: &str) -> PathBuf {
    let safe_id = Uuid::parse_str(id)
        .unwrap_or_else(|_| Uuid::nil())
        .to_string();
    state.data_dir.join("media").join(safe_id)
}
fn safe_mime(claimed: &str, bytes: &[u8]) -> &'static str {
    match claimed {
        "image/png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => "image/png",
        "image/jpeg" if bytes.starts_with(b"\xff\xd8\xff") => "image/jpeg",
        "image/gif" if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") => "image/gif",
        "image/webp" if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" => {
            "image/webp"
        }
        "application/pdf" if bytes.starts_with(b"%PDF-") => "application/pdf",
        "video/mp4" if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" => "video/mp4",
        "video/quicktime" if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" => "video/quicktime",
        "audio/mp4" if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" => "audio/mp4",
        "audio/mpeg" if bytes.starts_with(b"ID3") || bytes.starts_with(b"\xff\xfb") => "audio/mpeg",
        "audio/wav" if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" => {
            "audio/wav"
        }
        _ => "application/octet-stream",
    }
}
fn ensure_unlocked(state: &AppState) -> Result<(), String> {
    if state.unlocked.load(Ordering::SeqCst) {
        Ok(())
    } else {
        Err("앱 잠금을 먼저 해제하세요".into())
    }
}
fn save(state: &AppState, store: &Store) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(store).map_err(err)?;
    let dest = data_path(state);
    let temp = state.data_dir.join("journal.json.tmp");
    fs::write(&temp, json).map_err(err)?;
    fs::rename(&temp, &dest).map_err(err)?;
    Ok(())
}
fn mutate<T>(
    state: &AppState,
    op: impl FnOnce(&mut Store) -> Result<T, String>,
) -> Result<T, String> {
    ensure_unlocked(state)?;
    let mut guard = state.store.lock().map_err(err)?;
    let mut next = guard.clone();
    let result = op(&mut next)?;
    save(state, &next)?;
    *guard = next;
    Ok(result)
}

#[tauri::command]
fn bootstrap(state: State<'_, AppState>) -> Result<Snapshot, String> {
    let store = state.store.lock().map_err(err)?;
    let locked = !state.unlocked.load(Ordering::SeqCst);
    let drive_cfg = if locked {
        None
    } else {
        load_drive_config(&state.data_dir)
    };
    let connected = drive_cfg
        .as_ref()
        .is_some_and(|cfg| drive_auth::load_refresh_token(&cfg.client_id).is_ok());
    let has_sync_key = if locked {
        false
    } else {
        connected || load_local_sync_key(&state.data_dir).is_some()
    };
    let sync_status = if locked {
        SyncStatus::default()
    } else {
        SyncStatus {
            connected,
            has_oauth_client: drive_cfg.is_some(),
            client_id: drive_cfg.as_ref().map(|c| c.client_id.clone()),
            account_email: drive_cfg
                .as_ref()
                .filter(|_| connected)
                .and_then(|c| c.account_email.clone()),
            account_name: drive_cfg
                .as_ref()
                .filter(|_| connected)
                .and_then(|c| c.account_name.clone()),
            has_sync_key,
            sync_paused: store.sync_paused,
            last_sync_at: store.last_sync_at.clone(),
            last_sync_error: store.last_sync_error.clone(),
            pending_upload_count: sync_engine::pending_upload_count(&store),
            conflict_count: store.sync_conflicts.len(),
            pending_resolution_count: store.pending_resolutions.len(),
            remote_object_count: store.seen_remote_objects.len(),
            synced_media_bytes: store.media_manifests.values().map(|m| m.bytes).sum(),
        }
    };
    let synced_entry_ids = if locked {
        Vec::new()
    } else {
        store
            .entries
            .iter()
            .filter(|e| store.sync_base_entries.get(&e.id) == Some(*e))
            .map(|e| e.id.clone())
            .collect()
    };
    Ok(Snapshot {
        locked,
        lock_enabled: store.password_hash.is_some(),
        app_version: app_updater::CURRENT_VERSION.to_string(),
        installed_on_pc: app_installer::is_installed_on_pc(),
        running_installed: app_installer::is_running_from_installed_location(),
        install_path: app_installer::installed_exe_path()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        updater_config: if locked {
            app_updater::UpdaterConfig::default()
        } else {
            app_updater::effective_config(&store.updater_config)
        },
        nickname: if locked {
            String::new()
        } else {
            store.nickname.clone()
        },
        journals: if locked {
            Vec::new()
        } else {
            store.journals.clone()
        },
        entries: if locked {
            Vec::new()
        } else {
            store.entries.clone()
        },
        templates: if locked {
            Vec::new()
        } else {
            store.templates.clone()
        },
        sync_conflicts: if locked {
            Vec::new()
        } else {
            store.sync_conflicts.clone()
        },
        pending_resolution_count: if locked {
            0
        } else {
            store.pending_resolutions.len()
        },
        sync_status,
        synced_entry_ids,
    })
}

#[tauri::command]
fn resolve_sync_conflict(
    state: State<'_, AppState>,
    conflict_id: String,
    choice: String,
) -> Result<(), String> {
    mutate(&state, |store| {
        resolve_conflict(store, &state.data_dir, &conflict_id, &choice)
    })
}

fn resolve_conflict(
    store: &mut Store,
    data_dir: &std::path::Path,
    conflict_id: &str,
    choice: &str,
) -> Result<(), String> {
    if choice != "local" && choice != "remote" {
        return Err("잘못된 충돌 해결 선택입니다".into());
    }
    let index = store
        .sync_conflicts
        .iter()
        .position(|c| c.id == conflict_id)
        .ok_or("충돌을 찾을 수 없습니다")?;
    let conflict = store.sync_conflicts[index].clone();
    let current = store.entries.iter().position(|e| e.id == conflict.entry_id);
    if current.map(|i| &store.entries[i]) != conflict.local.as_ref() {
        return Err("충돌 이후 이 기록이 변경됐습니다. 다시 동기화하세요".into());
    }
    if choice == "remote" {
        if let Some(mut remote) = conflict.remote.clone() {
            if remote.id != conflict.entry_id
                || !valid_date(&remote.date)
                || !store.journals.iter().any(|j| j.id == remote.journal_id)
            {
                return Err("원격 기록이 올바르지 않습니다".into());
            }
            for attachment in &remote.attachments {
                let id = Uuid::parse_str(&attachment.id).map_err(|_| "잘못된 첨부 식별자")?;
                let metadata = fs::metadata(data_dir.join("media").join(id.to_string()))
                    .map_err(|_| "원격 첨부가 복원되지 않았습니다")?;
                if !metadata.is_file() || metadata.len() != attachment.bytes as u64 {
                    return Err("원격 첨부 크기가 일치하지 않습니다".into());
                }
            }
            remote.updated_at = now();
            if let Some(i) = current {
                store.entries[i] = remote;
            } else {
                store.entries.push(remote);
            }
        } else if let Some(i) = current {
            store.entries[i].deleted_at = Some(now());
            store.entries[i].updated_at = now();
        }
    }
    let chosen = store
        .entries
        .iter()
        .find(|entry| entry.id == conflict.entry_id)
        .cloned();
    store.pending_resolutions.push(PendingResolution {
        conflict,
        choice: choice.to_owned(),
        chosen,
    });
    store.sync_conflicts.remove(index);
    Ok(())
}

#[tauri::command]
fn unlock(state: State<'_, AppState>, password: String) -> Result<(), String> {
    let store = state.store.lock().map_err(err)?;
    if let Some(hash) = &store.password_hash {
        let parsed = PasswordHash::new(hash).map_err(err)?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| "암호가 맞지 않습니다".to_string())?;
    }
    state.unlocked.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn lock(state: State<'_, AppState>) -> Result<(), String> {
    let store = state.store.lock().map_err(err)?;
    if store.password_hash.is_some() {
        state.unlocked.store(false, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
fn set_lock_password(
    state: State<'_, AppState>,
    current_password: String,
    new_password: String,
) -> Result<(), String> {
    ensure_unlocked(&state)?;
    mutate(&state, |store| {
        if let Some(hash) = &store.password_hash {
            let parsed = PasswordHash::new(hash).map_err(err)?;
            Argon2::default()
                .verify_password(current_password.as_bytes(), &parsed)
                .map_err(|_| "현재 암호가 맞지 않습니다".to_string())?;
        }
        store.password_hash = if new_password.is_empty() {
            None
        } else {
            if new_password.chars().count() < 8 {
                return Err("암호는 8자 이상으로 설정하세요".into());
            }
            let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes()).map_err(err)?;
            Some(
                Argon2::default()
                    .hash_password(new_password.as_bytes(), &salt)
                    .map_err(err)?
                    .to_string(),
            )
        };
        Ok(())
    })
}

#[tauri::command]
fn set_nickname(state: State<'_, AppState>, nickname: String) -> Result<(), String> {
    let nickname = nickname.trim().to_string();
    if nickname.chars().count() > 40 {
        return Err("별명은 40자 이하로 입력해 주세요".into());
    }
    mutate(&state, |store| {
        store.nickname = nickname;
        Ok(())
    })
}

#[tauri::command]
fn updater_save_config(
    state: State<'_, AppState>,
    endpoint: String,
    pubkey: String,
    auto_check: bool,
) -> Result<(), String> {
    ensure_unlocked(&state)?;
    let endpoint = endpoint.trim().to_string();
    if !endpoint.is_empty() {
        app_updater::normalize_endpoint(&endpoint)?;
    }
    let pubkey = pubkey.trim().to_string();
    mutate(&state, |store| {
        store.updater_config.endpoint = endpoint;
        store.updater_config.pubkey = pubkey;
        store.updater_config.auto_check = auto_check;
        store.updater_config.skipped_version.clear();
        Ok(())
    })
}

#[tauri::command]
async fn updater_check(state: State<'_, AppState>) -> Result<app_updater::UpdateInfo, String> {
    ensure_unlocked(&state)?;
    let cfg = {
        let store = state.store.lock().map_err(err)?;
        store.updater_config.clone()
    };
    tauri::async_runtime::spawn_blocking(move || app_updater::check_update(&cfg))
        .await
        .map_err(err)?
}

#[tauri::command]
async fn updater_install(state: State<'_, AppState>) -> Result<String, String> {
    ensure_unlocked(&state)?;
    let cfg = {
        let store = state.store.lock().map_err(err)?;
        store.updater_config.clone()
    };
    tauri::async_runtime::spawn_blocking(move || app_updater::download_and_apply_update(&cfg))
        .await
        .map_err(err)?
}

#[tauri::command]
fn updater_skip_version(state: State<'_, AppState>, version: String) -> Result<(), String> {
    ensure_unlocked(&state)?;
    mutate(&state, |store| {
        store.updater_config.skipped_version = version.trim().to_string();
        Ok(())
    })
}

#[tauri::command]
fn updater_generate_keypair(
    state: State<'_, AppState>,
) -> Result<app_updater::SigningKeyPairInfo, String> {
    ensure_unlocked(&state)?;
    let kp = app_updater::generate_signing_keypair()?;
    let pubkey = kp.public_key_base64.clone();
    mutate(&state, |store| {
        store.updater_config.pubkey = pubkey;
        Ok(())
    })?;
    Ok(kp)
}

#[tauri::command]
fn updater_create_manifest(
    state: State<'_, AppState>,
    version: String,
    download_url: String,
    notes: String,
    private_key: String,
) -> Result<String, String> {
    ensure_unlocked(&state)?;
    app_installer::ensure_release_setup_binary();
    let exe_path = std::env::current_exe().map_err(err)?;
    let json = app_updater::sign_file_and_build_manifest(
        &exe_path,
        &version,
        &download_url,
        &notes,
        &private_key,
    )?;
    if let Some(dir) = exe_path.parent() {
        let _ = fs::write(dir.join("latest.json"), &json);
    }
    Ok(json)
}

#[tauri::command]
async fn install_app_to_pc(state: State<'_, AppState>) -> Result<String, String> {
    ensure_unlocked(&state)?;
    let path = tauri::async_runtime::spawn_blocking(|| app_installer::run_installer_ui(true))
        .await
        .map_err(err)??;
    if !app_installer::is_running_from_installed_location() {
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(500));
            std::process::exit(0);
        });
    }
    Ok(path.display().to_string())
}

#[tauri::command]
fn save_journal(
    state: State<'_, AppState>,
    id: Option<String>,
    name: String,
    color: String,
) -> Result<String, String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 80 {
        return Err("일지 이름은 1~80자여야 합니다".into());
    }
    if !valid_color(&color) {
        return Err("색상 형식이 잘못되었습니다".into());
    }
    mutate(&state, |store| {
        if let Some(id) = id {
            let journal = store
                .journals
                .iter_mut()
                .find(|j| j.id == id)
                .ok_or("일지를 찾을 수 없습니다")?;
            journal.name = name;
            journal.color = color;
            Ok(journal.id.clone())
        } else {
            let journal = Journal {
                name,
                color,
                ..Journal::default()
            };
            let id = journal.id.clone();
            store.journals.push(journal);
            Ok(id)
        }
    })
}

#[tauri::command]
fn delete_journal(state: State<'_, AppState>, id: String) -> Result<(), String> {
    mutate(&state, |store| {
        if store.entries.iter().any(|e| e.journal_id == id) {
            return Err(
                "글이 있는 일지는 삭제할 수 없습니다. 먼저 글을 이동하거나 삭제하세요".into(),
            );
        }
        if store.journals.len() <= 1 {
            return Err("마지막 일지는 삭제할 수 없습니다".into());
        }
        store.journals.retain(|j| j.id != id);
        Ok(())
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryInput {
    id: Option<String>,
    journal_id: String,
    title: String,
    body: String,
    date: String,
    favorite: bool,
    tags: Vec<String>,
    location: String,
    weather: String,
}

#[tauri::command]
fn save_entry(state: State<'_, AppState>, input: EntryInput) -> Result<String, String> {
    if !valid_date(&input.date) {
        return Err("날짜는 YYYY-MM-DD 형식이어야 합니다".into());
    }
    if input.title.len() > 500
        || input.body.len() > 2_000_000
        || input.location.len() > 1_000
        || input.weather.len() > 1_000
    {
        return Err("글 크기 한도를 넘었습니다".into());
    }
    mutate(&state, |store| {
        if !store.journals.iter().any(|j| j.id == input.journal_id) {
            return Err("일지를 찾을 수 없습니다".into());
        }
        let mut tags: Vec<String> = input
            .tags
            .into_iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty() && t.chars().count() <= 60)
            .collect();
        tags.sort();
        tags.dedup();
        tags.truncate(30);
        let timestamp = now();
        if let Some(id) = input.id {
            let entry = store
                .entries
                .iter_mut()
                .find(|e| e.id == id && e.deleted_at.is_none())
                .ok_or("글을 찾을 수 없습니다")?;
            entry.journal_id = input.journal_id;
            entry.title = input.title;
            entry.body = input.body;
            entry.date = input.date;
            entry.favorite = input.favorite;
            entry.tags = tags;
            entry.location = input.location;
            entry.weather = input.weather;
            entry.updated_at = timestamp;
            Ok(entry.id.clone())
        } else {
            let entry = Entry {
                journal_id: input.journal_id,
                title: input.title,
                body: input.body,
                date: input.date,
                favorite: input.favorite,
                tags,
                location: input.location,
                weather: input.weather,
                ..Entry::default()
            };
            let id = entry.id.clone();
            store.entries.push(entry);
            Ok(id)
        }
    })
}

#[tauri::command]
fn trash_entry(state: State<'_, AppState>, id: String) -> Result<(), String> {
    mutate(&state, |store| {
        let entry = store
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or("글을 찾을 수 없습니다")?;
        let ts = now();
        entry.deleted_at = Some(ts.clone());
        entry.updated_at = ts;
        Ok(())
    })
}

#[tauri::command]
fn restore_entry(state: State<'_, AppState>, id: String) -> Result<(), String> {
    mutate(&state, |store| {
        let entry = store
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or("글을 찾을 수 없습니다")?;
        entry.deleted_at = None;
        entry.updated_at = now();
        Ok(())
    })
}

#[tauri::command]
fn empty_trash(state: State<'_, AppState>) -> Result<usize, String> {
    let mut removed_attachments = Vec::new();
    let removed_count = mutate(&state, |store| {
        let trashed: Vec<Entry> = store
            .entries
            .iter()
            .filter(|e| e.deleted_at.is_some())
            .cloned()
            .collect();
        if trashed.is_empty() {
            return Ok(0);
        }
        let trashed_ids: std::collections::HashSet<String> =
            trashed.iter().map(|e| e.id.clone()).collect();
        store.entries.retain(|e| e.deleted_at.is_none());
        for id in &trashed_ids {
            store.sync_base_entries.remove(id);
            store.remote_heads.remove(id);
        }
        store
            .sync_conflicts
            .retain(|c| !trashed_ids.contains(&c.entry_id));
        store
            .pending_resolutions
            .retain(|r| !trashed_ids.contains(&r.conflict.entry_id));
        let remaining_att_ids: std::collections::HashSet<&str> = store
            .entries
            .iter()
            .flat_map(|e| e.attachments.iter().map(|a| a.id.as_str()))
            .collect();
        for entry in &trashed {
            for att in &entry.attachments {
                if !remaining_att_ids.contains(att.id.as_str()) {
                    store.media_manifests.remove(&att.id);
                    removed_attachments.push(att.id.clone());
                }
            }
        }
        Ok(trashed.len())
    })?;
    for att_id in removed_attachments {
        let _ = fs::remove_file(media_path(&state, &att_id));
    }
    Ok(removed_count)
}

#[tauri::command]
fn save_template(
    state: State<'_, AppState>,
    id: Option<String>,
    name: String,
    body: String,
) -> Result<String, String> {
    if name.trim().is_empty() || name.len() > 500 || body.len() > 2_000_000 {
        return Err("템플릿 크기 한도를 확인하세요".into());
    }
    mutate(&state, |store| {
        if let Some(id) = id {
            let template = store
                .templates
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or("템플릿을 찾을 수 없습니다")?;
            template.name = name;
            template.body = body;
            Ok(id)
        } else {
            let template = Template {
                name,
                body,
                ..Template::default()
            };
            let id = template.id.clone();
            store.templates.push(template);
            Ok(id)
        }
    })
}

#[tauri::command]
fn delete_template(state: State<'_, AppState>, id: String) -> Result<(), String> {
    mutate(&state, |store| {
        store.templates.retain(|t| t.id != id);
        Ok(())
    })
}

#[tauri::command]
fn add_attachment(
    state: State<'_, AppState>,
    entry_id: String,
    name: String,
    mime: String,
    data: String,
) -> Result<Attachment, String> {
    ensure_unlocked(&state)?;
    if data.len() > 140_000_000 {
        return Err("파일은 100 MB 이하만 첨부할 수 있습니다".into());
    }
    let bytes = B64.decode(data.as_bytes()).map_err(err)?;
    if bytes.len() > 100_000_000 {
        return Err("파일은 100 MB 이하만 첨부할 수 있습니다".into());
    }
    let mime = safe_mime(&mime, &bytes).to_string();
    let attachment = Attachment {
        id: Uuid::new_v4().to_string(),
        name: name.chars().take(200).collect(),
        mime,
        bytes: bytes.len(),
    };
    let path = media_path(&state, &attachment.id);
    fs::write(&path, bytes).map_err(err)?;
    let result = mutate(&state, |store| {
        let entry = store
            .entries
            .iter_mut()
            .find(|e| e.id == entry_id && e.deleted_at.is_none())
            .ok_or("글을 찾을 수 없습니다")?;
        entry.attachments.push(attachment.clone());
        entry.updated_at = now();
        Ok(attachment.clone())
    });
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}

#[tauri::command]
fn remove_attachment(
    state: State<'_, AppState>,
    entry_id: String,
    attachment_id: String,
) -> Result<(), String> {
    if Uuid::parse_str(&attachment_id).is_err() {
        return Err("잘못된 첨부 파일 식별자입니다".into());
    }
    mutate(&state, |store| {
        detach_attachment(store, &entry_id, &attachment_id)
    })?;
    match fs::remove_file(media_path(&state, &attachment_id)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "첨부 목록은 갱신됐지만 파일 삭제에 실패했습니다: {error}"
        )),
    }
}

fn detach_attachment(store: &mut Store, entry_id: &str, attachment_id: &str) -> Result<(), String> {
    let entry = store
        .entries
        .iter_mut()
        .find(|e| e.id == entry_id)
        .ok_or("글을 찾을 수 없습니다")?;
    let position = entry
        .attachments
        .iter()
        .position(|a| a.id == attachment_id)
        .ok_or("이 글의 첨부 파일을 찾을 수 없습니다")?;
    entry.attachments.remove(position);
    entry.updated_at = now();
    Ok(())
}

#[derive(Serialize)]
struct MediaData {
    mime: String,
    data: String,
}

#[tauri::command]
fn get_attachment(state: State<'_, AppState>, attachment_id: String) -> Result<MediaData, String> {
    ensure_unlocked(&state)?;
    let store = state.store.lock().map_err(err)?;
    let attachment = store
        .entries
        .iter()
        .flat_map(|e| &e.attachments)
        .find(|a| a.id == attachment_id)
        .ok_or("첨부 파일을 찾을 수 없습니다")?;
    let bytes = fs::read(media_path(&state, &attachment_id)).map_err(err)?;
    Ok(MediaData {
        mime: safe_mime(&attachment.mime, &bytes).to_string(),
        data: B64.encode(bytes),
    })
}

#[derive(Serialize, Deserialize)]
struct Backup {
    format: String,
    store: Store,
    media: HashMap<String, String>,
}

#[tauri::command]
fn export_backup(state: State<'_, AppState>, app: tauri::AppHandle) -> Result<String, String> {
    ensure_unlocked(&state)?;
    let mut store = state.store.lock().map_err(err)?.clone();
    store.password_hash = None;
    store.device_id.clear();
    store.sync_cursor = None;
    store.last_sync_at = None;
    store.last_sync_error = None;
    store.sync_base_entries.clear();
    store.sync_base_journals.clear();
    store.sync_base_templates.clear();
    store.sync_base_nickname = None;
    store.media_manifests.clear();
    store.seen_remote_objects.clear();
    store.remote_heads.clear();
    store.revision_parents.clear();
    let mut media = HashMap::new();
    for attachment in store.entries.iter().flat_map(|e| &e.attachments) {
        let bytes = fs::read(media_path(&state, &attachment.id)).map_err(err)?;
        media.insert(attachment.id.clone(), B64.encode(bytes));
    }
    let backup = Backup {
        format: "niki-journal-backup-v1".into(),
        store,
        media,
    };
    let dir = app
        .path()
        .document_dir()
        .map_err(err)?
        .join("Niki Journal Backups");
    fs::create_dir_all(&dir).map_err(err)?;
    let filename = format!(
        "niki-journal-{}-{}.json",
        Utc::now().format("%Y%m%d-%H%M%S"),
        Uuid::new_v4().simple()
    );
    let path = dir.join(filename);
    fs::write(&path, serde_json::to_vec_pretty(&backup).map_err(err)?).map_err(err)?;
    Ok(path.display().to_string())
}

#[derive(Serialize)]
struct ImportResult {
    entries_added: usize,
    entries_skipped: usize,
    media_added: usize,
    warnings: Vec<String>,
}

#[tauri::command]
fn import_data(
    state: State<'_, AppState>,
    name: String,
    data: String,
) -> Result<ImportResult, String> {
    ensure_unlocked(&state)?;
    if data.len() > 350_000_000 {
        return Err("가져오기 파일은 250 MB 이하만 지원합니다".into());
    }
    let bytes = B64.decode(data.as_bytes()).map_err(err)?;
    let files = if name.to_ascii_lowercase().ends_with(".zip") || bytes.starts_with(b"PK\x03\x04") {
        zip_read::read_zip(&bytes)?
    } else {
        vec![(name.clone(), bytes)]
    };
    let json_file = files
        .iter()
        .find(|(name, bytes)| {
            name.to_ascii_lowercase().ends_with(".json")
                && serde_json::from_slice::<serde_json::Value>(bytes)
                    .ok()
                    .is_some_and(|v| v.get("entries").is_some() || v.get("format").is_some())
        })
        .ok_or("일기 데이터 JSON을 찾을 수 없습니다")?;
    let value: serde_json::Value = serde_json::from_slice(&json_file.1).map_err(err)?;
    if value.get("format").and_then(|v| v.as_str()) == Some("niki-journal-backup-v1") {
        let backup: Backup = serde_json::from_value(value).map_err(err)?;
        import_own_backup(&state, backup)
    } else {
        import_day_one(&state, &name, &value, &files)
    }
}

fn import_own_backup(state: &AppState, backup: Backup) -> Result<ImportResult, String> {
    let mut added = 0;
    let mut skipped = 0;
    let mut media_added = 0;
    let mut written = Vec::new();
    let result = mutate(state, |store| {
        for mut journal in backup.store.journals {
            if !valid_color(&journal.color) {
                journal.color = "#576b61".into();
            }
            if !store.journals.iter().any(|j| j.id == journal.id) {
                store.journals.push(journal);
            }
        }
        for mut entry in backup.store.entries {
            if store.entries.iter().any(|e| e.id == entry.id) {
                skipped += 1;
                continue;
            }
            for attachment in &mut entry.attachments {
                let encoded = backup
                    .media
                    .get(&attachment.id)
                    .ok_or(format!("백업 미디어 누락: {}", attachment.name))?;
                let bytes = B64.decode(encoded.as_bytes()).map_err(err)?;
                if bytes.len() > 100_000_000 {
                    return Err("첨부 파일은 100 MB 이하만 가져올 수 있습니다".into());
                }
                attachment.mime = safe_mime(&attachment.mime, &bytes).to_string();
                attachment.bytes = bytes.len();
                attachment.id = Uuid::new_v4().to_string();
                let path = media_path(state, &attachment.id);
                fs::write(&path, bytes).map_err(err)?;
                written.push(path);
                media_added += 1;
            }
            store.entries.push(entry);
            added += 1;
        }
        for template in backup.store.templates {
            if !store.templates.iter().any(|t| t.id == template.id) {
                store.templates.push(template);
            }
        }
        Ok(())
    });
    if result.is_err() {
        for path in written {
            let _ = fs::remove_file(path);
        }
    }
    result?;
    Ok(ImportResult {
        entries_added: added,
        entries_skipped: skipped,
        media_added,
        warnings: Vec::new(),
    })
}

fn import_day_one(
    state: &AppState,
    archive_name: &str,
    value: &serde_json::Value,
    files: &[(String, Vec<u8>)],
) -> Result<ImportResult, String> {
    let source_entries = value
        .get("entries")
        .and_then(|v| v.as_array())
        .ok_or("Day One entries 배열이 없습니다")?;
    let journal_name = PathBuf::from(archive_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Day One 가져오기")
        .to_string();
    let mut added = 0;
    let mut skipped = 0;
    let mut media_added = 0;
    let mut warnings = Vec::new();
    let mut written = Vec::new();
    let result = mutate(state, |store| {
        let journal_id = if let Some(j) = store.journals.iter().find(|j| j.name == journal_name) {
            j.id.clone()
        } else {
            let journal = Journal {
                name: journal_name,
                ..Journal::default()
            };
            let id = journal.id.clone();
            store.journals.push(journal);
            id
        };
        for raw in source_entries {
            let external = raw.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
            if !external.is_empty()
                && store
                    .entries
                    .iter()
                    .any(|e| e.external_id.as_deref() == Some(external))
            {
                skipped += 1;
                continue;
            }
            let body = raw
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if body.is_empty()
                && (raw.get("richText").is_some()
                    || raw.get("encryptedContent").is_some()
                    || raw.get("encrypted-content").is_some())
            {
                warnings.push(format!(
                    "지원하지 않는 서식 본문을 건너뜀: {}",
                    if external.is_empty() {
                        "ID 없음"
                    } else {
                        external
                    }
                ));
                skipped += 1;
                continue;
            }
            let created_at = raw
                .get("creationDate")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let updated_at = raw
                .get("modifiedDate")
                .and_then(|v| v.as_str())
                .unwrap_or(&created_at)
                .to_string();
            let date = created_at.as_str();
            let date = date
                .get(..10)
                .filter(|v| valid_date(v))
                .map(str::to_owned)
                .unwrap_or_else(|| now()[..10].to_string());
            let tags = raw
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|tags| {
                    tags.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let title = body
                .lines()
                .next()
                .unwrap_or("")
                .trim_start_matches('#')
                .trim()
                .chars()
                .take(90)
                .collect();
            let location = raw
                .get("location")
                .and_then(|v| v.get("localityName").or_else(|| v.get("placeName")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let weather = raw
                .get("weather")
                .and_then(|v| v.get("weather").or_else(|| v.get("description")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let mut entry = Entry {
                journal_id: journal_id.clone(),
                title,
                body,
                date,
                tags,
                location,
                weather,
                created_at: if created_at.is_empty() {
                    now()
                } else {
                    created_at
                },
                updated_at: if updated_at.is_empty() {
                    now()
                } else {
                    updated_at
                },
                favorite: raw
                    .get("isFavorite")
                    .or_else(|| raw.get("starred"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                external_id: if external.is_empty() {
                    None
                } else {
                    Some(external.to_string())
                },
                ..Entry::default()
            };
            for (field, prefix) in [
                ("photos", "photos"),
                ("videos", "videos"),
                ("audios", "audios"),
                ("pdfs", "pdfs"),
            ] {
                if let Some(items) = raw.get(field).and_then(|v| v.as_array()) {
                    for item in items {
                        let identifier = item
                            .get("identifier")
                            .or_else(|| item.get("uuid"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if identifier.is_empty() {
                            warnings.push(format!("{field} 식별자 누락"));
                            continue;
                        }
                        let found = files.iter().find(|(path, _)| {
                            let normalized = path.replace('\\', "/").to_ascii_lowercase();
                            normalized.contains(prefix)
                                && PathBuf::from(path)
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .is_some_and(|stem| stem.eq_ignore_ascii_case(identifier))
                        });
                        if let Some((path, bytes)) = found {
                            let mime = mime_from_path(path);
                            let attachment = Attachment {
                                id: Uuid::new_v4().to_string(),
                                name: PathBuf::from(path)
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("media")
                                    .to_string(),
                                mime,
                                bytes: bytes.len(),
                            };
                            let dest = media_path(state, &attachment.id);
                            fs::write(&dest, bytes).map_err(err)?;
                            written.push(dest);
                            entry.attachments.push(attachment);
                            media_added += 1;
                        } else {
                            warnings.push(format!("미디어 파일 누락: {identifier}"));
                        }
                    }
                }
            }
            store.entries.push(entry);
            added += 1;
        }
        Ok(())
    });
    if result.is_err() {
        for path in written {
            let _ = fs::remove_file(path);
        }
    }
    result?;
    Ok(ImportResult {
        entries_added: added,
        entries_skipped: skipped,
        media_added,
        warnings,
    })
}

fn mime_from_path(path: &str) -> String {
    match PathBuf::from(path)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "m4a" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
    .into()
}

pub fn run() {
    app_updater::cleanup_old_executable();
    app_installer::ensure_release_setup_binary();
    match app_installer::detect_startup_mode() {
        app_installer::StartupMode::Install { silent } => {
            if let Err(e) = app_installer::run_installer_ui(silent)
                && !silent
                && !e.contains("취소")
            {
                app_installer::error_dialog("Niki 설치 오류", &e);
            }
            return;
        }
        app_installer::StartupMode::Uninstall => {
            if let Err(e) = app_installer::run_uninstaller_ui() {
                app_installer::error_dialog("Niki 제거 오류", &e);
            }
            return;
        }
        app_installer::StartupMode::NormalApp => {}
    }
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = match std::env::var_os("NIKI_JOURNAL_DATA_DIR") {
                Some(path) => PathBuf::from(path),
                None => app.path().app_data_dir()?,
            };
            fs::create_dir_all(data_dir.join("media"))?;
            let path = data_dir.join("journal.json");
            let mut store = if path.exists() {
                serde_json::from_slice::<Store>(&fs::read(path)?)?
            } else {
                Store {
                    version: 1,
                    journals: vec![Journal {
                        name: "나의 일지".into(),
                        ..Journal::default()
                    }],
                    ..Store::default()
                }
            };
            for journal in &mut store.journals {
                if !valid_color(&journal.color) {
                    journal.color = "#576b61".into();
                }
            }
            let unlocked = store.password_hash.is_none();
            if let Some(icon) = app.default_window_icon().cloned()
                && let Some(window) = app.get_webview_window("main")
            {
                let _ = window.set_icon(icon);
            }
            app.manage(AppState {
                data_dir,
                store: Mutex::new(store),
                unlocked: AtomicBool::new(unlocked),
                syncing: AtomicBool::new(false),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            unlock,
            lock,
            set_lock_password,
            set_nickname,
            updater_save_config,
            updater_check,
            updater_install,
            updater_skip_version,
            updater_generate_keypair,
            updater_create_manifest,
            install_app_to_pc,
            save_journal,
            delete_journal,
            save_entry,
            trash_entry,
            restore_entry,
            empty_trash,
            save_template,
            delete_template,
            add_attachment,
            remove_attachment,
            get_attachment,
            export_backup,
            import_data,
            drive_connect,
            drive_disconnect,
            drive_generate_sync_key,
            drive_get_sync_key,
            drive_set_sync_key,
            drive_set_paused,
            drive_sync_now,
            resolve_sync_conflict
        ])
        .run(tauri::generate_context!())
        .expect("앱을 시작할 수 없습니다");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflict_resolution_preserves_changed_local_and_rejects_missing_media() {
        let journal = Journal::default();
        let local = Entry {
            journal_id: journal.id.clone(),
            body: "local".into(),
            ..Entry::default()
        };
        let mut remote = local.clone();
        remote.body = "remote".into();
        let conflict = SyncConflict {
            id: "conflict".into(),
            entry_id: local.id.clone(),
            local: Some(local.clone()),
            remote: Some(remote.clone()),
        };
        let mut store = Store {
            journals: vec![journal],
            entries: vec![local.clone()],
            sync_conflicts: vec![conflict],
            ..Store::default()
        };
        store.entries[0].body = "changed".into();
        assert!(
            resolve_conflict(&mut store, std::path::Path::new("."), "conflict", "remote").is_err()
        );
        assert_eq!(store.entries[0].body, "changed");
        assert_eq!(store.sync_conflicts.len(), 1);
        store.entries[0] = local;
        store.sync_conflicts[0]
            .remote
            .as_mut()
            .unwrap()
            .attachments
            .push(Attachment {
                id: Uuid::new_v4().to_string(),
                bytes: 10,
                ..Attachment::default()
            });
        assert!(
            resolve_conflict(&mut store, std::path::Path::new("."), "conflict", "remote").is_err()
        );
        assert_eq!(store.entries[0].body, "local");
        assert_eq!(store.sync_conflicts.len(), 1);
        store.sync_conflicts[0].remote = Some(remote);
        resolve_conflict(&mut store, std::path::Path::new("."), "conflict", "remote").unwrap();
        assert_eq!(store.entries[0].body, "remote");
        assert!(store.sync_conflicts.is_empty());
        assert_eq!(store.pending_resolutions.len(), 1);
        assert_eq!(
            store.pending_resolutions[0]
                .conflict
                .local
                .as_ref()
                .unwrap()
                .body,
            "local"
        );
        assert_eq!(
            store.pending_resolutions[0].chosen.as_ref().unwrap().body,
            "remote"
        );
        let round_trip: Store =
            serde_json::from_slice(&serde_json::to_vec(&store).unwrap()).unwrap();
        assert_eq!(round_trip.pending_resolutions.len(), 1);
    }
    #[test]
    fn validates_calendar_dates() {
        assert!(valid_date("2026-10-29"));
        assert!(!valid_date("2026-02-30"));
    }
    #[test]
    fn maps_common_media_types() {
        assert_eq!(mime_from_path("photos/A.JPEG"), "image/jpeg");
        assert_eq!(mime_from_path("pdfs/A.pdf"), "application/pdf");
    }
    #[test]
    fn rejects_untrusted_mime_and_mismatched_file_signature() {
        assert_eq!(
            safe_mime("image/png\" onerror=\"alert(1)", b"\x89PNG\r\n\x1a\n"),
            "application/octet-stream"
        );
        assert_eq!(
            safe_mime("image/svg+xml", b"<svg onload='alert(1)'>"),
            "application/octet-stream"
        );
        assert_eq!(
            safe_mime("image/png", b"<script>"),
            "application/octet-stream"
        );
        assert_eq!(safe_mime("image/png", b"\x89PNG\r\n\x1a\n"), "image/png");
    }
    #[test]
    fn cannot_detach_media_from_another_entry() {
        let attachment = Attachment {
            id: Uuid::new_v4().to_string(),
            ..Attachment::default()
        };
        let first = Entry {
            id: "first".into(),
            ..Entry::default()
        };
        let second = Entry {
            id: "second".into(),
            attachments: vec![attachment.clone()],
            ..Entry::default()
        };
        let mut store = Store {
            entries: vec![first, second],
            ..Store::default()
        };
        assert!(detach_attachment(&mut store, "first", &attachment.id).is_err());
        assert_eq!(store.entries[1].attachments.len(), 1);
        assert!(detach_attachment(&mut store, "second", &attachment.id).is_ok());
        assert!(store.entries[1].attachments.is_empty());
    }
    #[test]
    fn imports_day_one_text_and_skips_duplicate_uuid() {
        let test_root = std::env::current_dir().unwrap().join("dev-data");
        fs::create_dir_all(&test_root).unwrap();
        let dir = test_root.join(format!("niki-journal-test-{}", Uuid::new_v4()));
        fs::create_dir_all(dir.join("media")).unwrap();
        let state = AppState {
            data_dir: dir.clone(),
            store: Mutex::new(Store {
                version: 1,
                journals: vec![Journal::default()],
                ..Store::default()
            }),
            unlocked: AtomicBool::new(true),
            syncing: AtomicBool::new(false),
        };
        let value = serde_json::json!({"entries": [{"uuid":"sample-1", "text":"# 산책\n오늘 걸었다", "creationDate":"2025-05-03T12:00:00Z", "tags":["산책"]}]});
        let first = import_day_one(&state, "sample.json", &value, &[]).unwrap();
        let second = import_day_one(&state, "sample.json", &value, &[]).unwrap();
        assert_eq!((first.entries_added, second.entries_skipped), (1, 1));
        let saved = state.store.lock().unwrap();
        assert_eq!(saved.entries[0].date, "2025-05-03");
        assert_eq!(saved.entries[0].body, "# 산책\n오늘 걸었다");
        drop(saved);
        fs::remove_file(dir.join("journal.json")).unwrap();
        fs::remove_dir(dir.join("media")).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
