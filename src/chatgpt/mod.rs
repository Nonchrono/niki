//! ChatGPT subscription connection. Credentials never enter journal data, backups, or the webview.
mod auth;
mod inference;
mod storage;

use crate::{AppState, ensure_unlocked};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::State;

const MODEL: &str = "gpt-6-luna";
const RESOURCE: &str = "https://api.openai.com/v1";
const ISSUER: &str = "https://auth.openai.com";
const TOKEN: &str = "https://auth.openai.com/api/accounts/oauth/token";
const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";

#[derive(Default, Clone)]
pub(crate) struct Tasks(Arc<Mutex<Option<(String, Arc<AtomicBool>)>>>);
struct Operation {
    tasks: Tasks,
    cancelled: Arc<AtomicBool>,
    _file: fs::File,
}
impl Drop for Operation {
    fn drop(&mut self) {
        if let Ok(mut task) = self.tasks.0.lock() {
            *task = None;
        }
    }
}
impl Operation {
    fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::SeqCst) {
            Err("요청을 취소했습니다".into())
        } else {
            Ok(())
        }
    }
}
impl Tasks {
    fn start(&self, dir: &Path, id: &str) -> Result<Operation, String> {
        uuid::Uuid::parse_str(id).map_err(|_| "잘못된 요청 식별자")?;
        let mut task = self.0.lock().map_err(|_| "ChatGPT 작업 상태 오류")?;
        if task.is_some() {
            return Err(
                "진행 중인 ChatGPT 요청이 있습니다. 완료 또는 취소 후 다시 시도하세요.".into(),
            );
        }
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join("chatgpt.lock"))
            .map_err(|_| "ChatGPT 연결 잠금 파일을 열지 못했습니다")?;
        file.try_lock()
            .map_err(|_| "다른 Niki 창에서 ChatGPT를 사용 중입니다. 잠시 후 다시 시도하세요.")?;
        let cancelled = Arc::new(AtomicBool::new(false));
        *task = Some((id.into(), cancelled.clone()));
        Ok(Operation {
            tasks: self.clone(),
            cancelled,
            _file: file,
        })
    }
    pub(crate) fn cancel(&self, id: Option<&str>) {
        if let Ok(task) = self.0.lock()
            && let Some((active, cancelled)) = task.as_ref()
            && id.is_none_or(|id| id == active)
        {
            cancelled.store(true, Ordering::SeqCst);
        }
    }
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Niki/ChatGPT")
        .build()
        .map_err(|_| "ChatGPT 통신을 초기화하지 못했습니다".into())
}

#[derive(Serialize)]
pub(crate) struct AccountInfo {
    client_id: String,
    email: String,
    connected: bool,
    plan_enabled: bool,
    welcome_seen: bool,
}
#[derive(Serialize)]
pub(crate) struct Status {
    accounts: Vec<AccountInfo>,
    active: Option<String>,
    model: &'static str,
}
fn status(accounts: &storage::Accounts) -> Status {
    Status {
        active: accounts.active.clone(),
        model: MODEL,
        accounts: accounts
            .accounts
            .iter()
            .map(|a| AccountInfo {
                client_id: a.client_id.clone(),
                email: a.email.clone(),
                connected: a.credentials.is_some(),
                welcome_seen: a.welcome_seen,
                plan_enabled: a.credentials.as_ref().is_some_and(|c| has_plan(&c.scopes)),
            })
            .collect(),
    }
}
fn has_plan(scopes: &str) -> bool {
    scopes
        .split_whitespace()
        .any(|s| s == "chatgpt.tokens.use.direct")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tasks_serialize_across_instances_and_cancel_only_the_owner() {
        let dir = std::env::temp_dir().join(format!("niki-chatgpt-lock-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let first = Tasks::default();
        let second = Tasks::default();
        let id = uuid::Uuid::new_v4().to_string();
        let other = uuid::Uuid::new_v4().to_string();
        let op = first.start(&dir, &id).unwrap();
        assert!(first.start(&dir, &other).is_err());
        assert!(second.start(&dir, &other).is_err());
        first.cancel(Some(&other));
        assert!(op.check().is_ok());
        first.cancel(Some(&id));
        assert!(op.check().is_err());
        assert!(second.start(&dir, &other).is_err());
        drop(op);
        drop(second.start(&dir, &other).unwrap());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn scope_matching_is_exact_and_status_never_contains_credentials() {
        assert!(!has_plan("chatgpt.tokens.use.direct.extra"));
        assert!(has_plan(SCOPES));
        let accounts = storage::Accounts {
            pending_client_id: None,
            active: Some("client".into()),
            accounts: vec![storage::Account {
                credentials: Some(storage::Credentials {
                    access_token: "SECRET_ACCESS".into(),
                    refresh_token: "SECRET_REFRESH".into(),
                    id_token: "SECRET_ID".into(),
                    scopes: SCOPES.into(),
                    ..Default::default()
                }),
                ..Default::default()
            }],
        };
        let json = serde_json::to_string(&status(&accounts)).unwrap();
        assert!(!json.contains("SECRET"));
        assert!(json.contains("\"plan_enabled\":true"));
    }
}

#[tauri::command]
pub(crate) async fn chatgpt_status(state: State<'_, AppState>) -> Result<Status, String> {
    ensure_unlocked(&state)?;
    let dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || storage::load(&dir).map(|s| status(&s)))
        .await
        .map_err(|_| "ChatGPT 상태 조회 실패")?
}

#[tauri::command]
pub(crate) async fn chatgpt_connect(
    state: State<'_, AppState>,
    tasks: State<'_, Tasks>,
    request_id: String,
    client_id: Option<String>,
) -> Result<Status, String> {
    ensure_unlocked(&state)?;
    let operation = tasks.start(&state.data_dir, &request_id)?;
    let dir = state.data_dir.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut accounts = storage::load(&dir)?;
        let previous = match client_id.as_deref() {
            Some(id) => Some(
                accounts
                    .accounts
                    .iter()
                    .find(|a| a.client_id == id)
                    .ok_or("저장된 ChatGPT 계정을 찾지 못했습니다")?
                    .clone(),
            ),
            None => None,
        };
        let host = storage::host_id(&dir)?;
        let pending = accounts.pending_client_id.clone();
        let account = auth::connect(
            &client()?,
            &host,
            previous.as_ref(),
            pending.as_deref(),
            &operation,
            |id| {
                if previous.is_none() {
                    accounts.pending_client_id = Some(id.into());
                    storage::save(&dir, &accounts)?;
                }
                Ok(())
            },
        )?;
        operation.check()?;
        if accounts.pending_client_id.as_deref() == Some(&account.client_id) {
            accounts.pending_client_id = None;
        }
        accounts.active = Some(account.client_id.clone());
        if let Some(saved) = accounts
            .accounts
            .iter_mut()
            .find(|a| a.client_id == account.client_id)
        {
            if saved.subject != account.subject {
                return Err("ChatGPT 계정 정보가 일치하지 않습니다".into());
            }
            *saved = account;
        } else {
            accounts.accounts.push(account);
        }
        storage::save(&dir, &accounts)?;
        Ok(status(&accounts))
    })
    .await
    .map_err(|_| "ChatGPT 연결 작업 실패")?;
    ensure_unlocked(&state)?;
    result
}

#[tauri::command]
pub(crate) fn chatgpt_cancel(tasks: State<'_, Tasks>, request_id: String) {
    tasks.cancel(Some(&request_id));
}

#[tauri::command]
pub(crate) async fn chatgpt_select_account(
    state: State<'_, AppState>,
    tasks: State<'_, Tasks>,
    client_id: String,
) -> Result<Status, String> {
    ensure_unlocked(&state)?;
    let operation = tasks.start(&state.data_dir, &uuid::Uuid::new_v4().to_string())?;
    let dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let mut accounts = storage::load(&dir)?;
        if !accounts.accounts.iter().any(|a| a.client_id == client_id) {
            return Err("ChatGPT 계정을 찾지 못했습니다".into());
        }
        accounts.active = Some(client_id);
        storage::save(&dir, &accounts)?;
        Ok(status(&accounts))
    })
    .await
    .map_err(|_| "ChatGPT 계정 선택 실패")?
}

#[tauri::command]
pub(crate) async fn chatgpt_acknowledge(
    state: State<'_, AppState>,
    tasks: State<'_, Tasks>,
) -> Result<(), String> {
    ensure_unlocked(&state)?;
    let operation = tasks.start(&state.data_dir, &uuid::Uuid::new_v4().to_string())?;
    let dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let mut accounts = storage::load(&dir)?;
        let id = accounts.active.clone();
        if let Some(account) = accounts
            .accounts
            .iter_mut()
            .find(|a| Some(&a.client_id) == id.as_ref())
        {
            account.welcome_seen = true;
        }
        storage::save(&dir, &accounts)
    })
    .await
    .map_err(|_| "ChatGPT 설정 저장 실패")?
}

#[tauri::command]
pub(crate) async fn chatgpt_disconnect(
    state: State<'_, AppState>,
    tasks: State<'_, Tasks>,
) -> Result<String, String> {
    ensure_unlocked(&state)?;
    let operation = tasks.start(&state.data_dir, &uuid::Uuid::new_v4().to_string())?;
    let dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let mut accounts = storage::load(&dir)?;
        let id = accounts.active.clone();
        let Some(account) = accounts.accounts.iter_mut().find(|a| Some(&a.client_id) == id.as_ref()) else { return Ok("연결된 ChatGPT 계정이 없습니다".into()); };
        let revoked = account.credentials.as_ref().is_none_or(|c| auth::revoke(&account.client_id, &c.refresh_token));
        account.credentials = None;
        storage::save(&dir, &accounts)?;
        Ok(if revoked { "ChatGPT 연결을 해제했습니다" } else { "이 기기에서 로그아웃했습니다. 원격 세션 해제를 확인하지 못했으므로 ChatGPT 설정에서도 Niki 연결을 해제해 주세요." }.into())
    }).await.map_err(|_| "ChatGPT 연결 해제 실패")?
}

#[tauri::command]
pub(crate) fn chatgpt_open_usage(state: State<'_, AppState>) -> Result<(), String> {
    ensure_unlocked(&state)?;
    auth::open_browser("https://chatgpt.com/settings/usage")
}

#[tauri::command]
pub(crate) async fn chatgpt_summarize(
    state: State<'_, AppState>,
    tasks: State<'_, Tasks>,
    request_id: String,
    entry_id: String,
    expected_updated_at: String,
) -> Result<inference::Summary, String> {
    ensure_unlocked(&state)?;
    let operation = tasks.start(&state.data_dir, &request_id)?;
    let entry = {
        let store = state.store.lock().map_err(|_| "일기를 읽지 못했습니다")?;
        let entry = store
            .entries
            .iter()
            .find(|e| e.id == entry_id && e.deleted_at.is_none())
            .ok_or("요약할 일기를 찾지 못했습니다")?;
        if entry.updated_at != expected_updated_at {
            return Err("일기가 변경되었습니다. 최신 내용을 확인하고 다시 요약해 주세요.".into());
        }
        entry.clone()
    };
    let payload = inference::summary_request(&entry)?;
    let dir = state.data_dir.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut accounts = storage::load(&dir)?;
        let id = accounts.active.clone();
        let account = accounts
            .accounts
            .iter_mut()
            .find(|a| Some(&a.client_id) == id.as_ref())
            .ok_or("설정에서 ChatGPT를 먼저 연결하세요")?;
        let http = client()?;
        let token = auth::access_token(&http, account);
        // Persist rotated credentials (or confirmed invalidation) before inference.
        storage::save(&dir, &accounts)?;
        let token = token?;
        operation.check()?;
        inference::summarize(&http, &token, payload, &operation)
    })
    .await
    .map_err(|_| "일기 요약 작업 실패")?;
    ensure_unlocked(&state)?;
    result
}
