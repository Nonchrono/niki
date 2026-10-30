//! Encrypted Google Drive synchronization engine.

use crate::{
    Entry, Journal, PendingUpload, Store, SyncConflict, Template,
    drive_transport::RemoteTransport,
    err, now, safe_mime,
    sync_media::{self, Manifest},
    sync_merge::{self, Decision},
    valid_color, valid_date,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    path::Path,
};
use uuid::Uuid;

const REV_PREFIX: [u8; 6] = *b"NIKISY";
const REV_UUID_PREFIX: &str = "4e494b49-5359-";

pub fn new_revision_uuid() -> Uuid {
    let mut bytes = *Uuid::new_v4().as_bytes();
    bytes[..6].copy_from_slice(&REV_PREFIX);
    Uuid::from_bytes(bytes)
}

pub fn parse_revision_uuid(name: &str) -> Option<Uuid> {
    if !name.starts_with(REV_UUID_PREFIX) {
        return None;
    }
    Uuid::parse_str(name).ok()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncRevision {
    pub version: u32,
    pub object_id: Uuid,
    pub parent_ids: Vec<String>,
    pub device_id: String,
    pub created_at: String,
    pub entry: Option<Entry>,
    pub journal: Option<Journal>,
    #[serde(default)]
    pub journals: Vec<Journal>,
    #[serde(default)]
    pub templates: Vec<Template>,
    #[serde(default)]
    pub deleted_journal_ids: Vec<String>,
    #[serde(default)]
    pub deleted_template_ids: Vec<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub media_manifests: HashMap<String, Manifest>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncSummary {
    pub downloaded_revisions: usize,
    pub uploaded_revisions: usize,
    pub uploaded_media: usize,
    pub restored_media: usize,
    pub conflicts_created: usize,
    pub warnings: Vec<String>,
}

pub fn pending_upload_count(store: &Store) -> usize {
    let conflicted: HashSet<&str> = store
        .sync_conflicts
        .iter()
        .map(|c| c.entry_id.as_str())
        .collect();
    let entry_changes = store
        .entries
        .iter()
        .filter(|e| {
            !conflicted.contains(e.id.as_str()) && store.sync_base_entries.get(&e.id) != Some(*e)
        })
        .count();
    let journal_changes = store
        .journals
        .iter()
        .filter(|j| store.sync_base_journals.get(&j.id) != Some(*j))
        .count()
        + store
            .sync_base_journals
            .keys()
            .filter(|id| !store.journals.iter().any(|j| &j.id == *id))
            .count();
    let template_changes = store
        .templates
        .iter()
        .filter(|t| store.sync_base_templates.get(&t.id) != Some(*t))
        .count()
        + store
            .sync_base_templates
            .keys()
            .filter(|id| !store.templates.iter().any(|t| &t.id == *id))
            .count();
    let nickname_changes = if store.sync_base_nickname.as_deref() != Some(store.nickname.as_str()) {
        1
    } else {
        0
    };
    entry_changes
        + journal_changes
        + template_changes
        + nickname_changes
        + store.pending_resolutions.len()
        + store.pending_uploads.len()
}

fn is_ancestor(
    parents_map: &HashMap<String, Vec<String>>,
    ancestor_id: &str,
    descendant_id: &str,
) -> bool {
    if ancestor_id == descendant_id {
        return true;
    }
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(descendant_id);
    while let Some(current) = queue.pop_front() {
        if !visited.insert(current) || visited.len() > 10_000 {
            continue;
        }
        if let Some(parents) = parents_map.get(current) {
            for parent in parents {
                if parent == ancestor_id {
                    return true;
                }
                queue.push_back(parent.as_str());
            }
        }
    }
    false
}

fn merge_journal(store: &mut Store, id: &str, remote: Option<Journal>, warnings: &mut Vec<String>) {
    let remote = remote.map(|mut journal| {
        journal.name = journal.name.trim().to_string();
        if !valid_color(&journal.color) {
            journal.color = "#576b61".into();
        }
        journal
    });
    let base = store.sync_base_journals.get(id);
    let local_idx = store.journals.iter().position(|j| j.id == id);
    let local = local_idx.map(|i| &store.journals[i]);
    match sync_merge::merge(base, local, remote.as_ref()) {
        Decision::Merged(merged) => {
            if let Some(i) = local_idx {
                store.journals.remove(i);
            }
            if let Some(value) = merged {
                store.journals.push(value);
            }
        }
        Decision::Conflict {
            local,
            remote: conflicting_remote,
        } => {
            if let Some(mut copy) = local {
                copy.id = Uuid::new_v4().to_string();
                copy.name = format!("{} (복사본)", copy.name);
                store.journals.push(copy);
            }
            if let Some(i) = local_idx {
                store.journals.remove(i);
            }
            if let Some(value) = conflicting_remote {
                store.journals.push(value);
            }
            warnings.push(format!(
                "일지 '{}'의 동시 변경을 복사본으로 보존했습니다.",
                id
            ));
        }
    }
    if let Some(value) = remote {
        store.sync_base_journals.insert(id.to_owned(), value);
    } else {
        store.sync_base_journals.remove(id);
    }
}

fn merge_template(
    store: &mut Store,
    id: &str,
    remote: Option<Template>,
    warnings: &mut Vec<String>,
) {
    let base = store.sync_base_templates.get(id);
    let local_idx = store.templates.iter().position(|t| t.id == id);
    let local = local_idx.map(|i| &store.templates[i]);
    match sync_merge::merge(base, local, remote.as_ref()) {
        Decision::Merged(merged) => {
            if let Some(i) = local_idx {
                store.templates.remove(i);
            }
            if let Some(value) = merged {
                store.templates.push(value);
            }
        }
        Decision::Conflict {
            local,
            remote: conflicting_remote,
        } => {
            if let Some(mut copy) = local {
                copy.id = Uuid::new_v4().to_string();
                copy.name = format!("{} (복사본)", copy.name);
                store.templates.push(copy);
            }
            if let Some(i) = local_idx {
                store.templates.remove(i);
            }
            if let Some(value) = conflicting_remote {
                store.templates.push(value);
            }
            warnings.push(format!(
                "템플릿 '{}'의 동시 변경을 복사본으로 보존했습니다.",
                id
            ));
        }
    }
    if let Some(value) = remote {
        store.sync_base_templates.insert(id.to_owned(), value);
    } else {
        store.sync_base_templates.remove(id);
    }
}

fn merge_nickname(store: &mut Store, remote: String, warnings: &mut Vec<String>) {
    let remote = remote.trim();
    if remote.chars().count() > 40 {
        return;
    }
    let base = store.sync_base_nickname.as_deref();
    let local = store.nickname.as_str();
    if base.is_none() && local.is_empty() || base == Some(local) {
        store.nickname = remote.to_string();
    } else if local != remote && base != Some(remote) {
        warnings.push("별명이 다른 기기에서도 변경되어 이 기기의 별명을 유지했습니다.".into());
    }
    store.sync_base_nickname = Some(remote.to_string());
}

fn valid_remote_journal(j: &Journal) -> bool {
    !j.id.is_empty()
        && j.id.len() <= 100
        && !j.name.trim().is_empty()
        && j.name.chars().count() <= 80
}

fn valid_remote_template(t: &Template) -> bool {
    !t.id.is_empty()
        && t.id.len() <= 100
        && !t.name.trim().is_empty()
        && t.name.len() <= 500
        && t.body.len() <= 2_000_000
}

fn validate_remote_entry(entry: &Entry) -> Result<(), String> {
    if entry.id.is_empty() || entry.id.len() > 100 || entry.journal_id.is_empty() {
        return Err("원격 기록 식별자가 올바르지 않습니다".into());
    }
    if !valid_date(&entry.date) {
        return Err("원격 기록 날짜 형식이 올바르지 않습니다".into());
    }
    if entry.title.len() > 500
        || entry.body.len() > 2_000_000
        || entry.location.len() > 1_000
        || entry.weather.len() > 1_000
        || entry.tags.len() > 50
        || entry.attachments.len() > 200
    {
        return Err("원격 기록 크기 한도를 초과했습니다".into());
    }
    Ok(())
}

fn restore_entry_media(
    data_dir: &Path,
    transport: &impl RemoteTransport,
    manifests: &HashMap<String, Manifest>,
    entry: &mut Entry,
    restored_count: &mut usize,
) -> Result<(), String> {
    let media_dir = data_dir.join("media");
    fs::create_dir_all(&media_dir).map_err(err)?;
    for attachment in &mut entry.attachments {
        let id = Uuid::parse_str(&attachment.id).map_err(|_| "잘못된 원격 첨부 식별자")?;
        if attachment.bytes > 100_000_000 {
            return Err("원격 첨부 파일 크기 한도 초과".into());
        }
        let manifest = manifests
            .get(&attachment.id)
            .ok_or_else(|| format!("원격 첨부 목록 누락: {}", attachment.name))?;
        if manifest.bytes != attachment.bytes {
            return Err("원격 첨부 크기 불일치".into());
        }
        let dest = media_dir.join(id.to_string());
        let existing_bytes = fs::read(&dest).ok();
        let already_valid = existing_bytes.as_ref().is_some_and(|existing| {
            existing.len() == manifest.bytes && sync_media::hash(existing) == manifest.sha256
        });
        if let Some(existing) = existing_bytes
            && already_valid
        {
            attachment.mime = safe_mime(&attachment.mime, &existing).to_string();
        } else {
            let bytes = sync_media::restore(transport, manifest)?;
            attachment.mime = safe_mime(&attachment.mime, &bytes).to_string();
            let temp = media_dir.join(format!("{id}.tmp"));
            fs::write(&temp, &bytes).map_err(err)?;
            fs::rename(&temp, &dest).map_err(err)?;
            *restored_count += 1;
        }
    }
    Ok(())
}

fn conflict_remote_entry(entry: &Entry) -> Option<Entry> {
    if entry.deleted_at.is_some() {
        None
    } else {
        Some(entry.clone())
    }
}

fn upsert_conflict(store: &mut Store, entry_id: &str, local: Option<Entry>, remote: Option<Entry>) {
    if let Some(existing) = store
        .sync_conflicts
        .iter_mut()
        .find(|c| c.entry_id == entry_id)
    {
        existing.local = local;
        existing.remote = remote;
    } else {
        store.sync_conflicts.push(SyncConflict {
            id: Uuid::new_v4().to_string(),
            entry_id: entry_id.to_string(),
            local,
            remote,
        });
    }
}

fn acknowledge_upload(store: &mut Store, pending: &PendingUpload, file_id: String) {
    let rev = &pending.revision;
    let rev_id = rev.object_id.to_string();
    store.seen_remote_objects.insert(rev_id.clone(), file_id);
    store
        .revision_parents
        .insert(rev_id.clone(), rev.parent_ids.clone());
    if let Some(entry) = &rev.entry {
        store
            .remote_heads
            .insert(entry.id.clone(), vec![rev_id.clone()]);
        store
            .sync_base_entries
            .insert(entry.id.clone(), entry.clone());
    }
    if let Some(journal) = &rev.journal {
        store
            .sync_base_journals
            .insert(journal.id.clone(), journal.clone());
    }
    for journal in &rev.journals {
        store
            .sync_base_journals
            .insert(journal.id.clone(), journal.clone());
    }
    for template in &rev.templates {
        store
            .sync_base_templates
            .insert(template.id.clone(), template.clone());
    }
    for id in &rev.deleted_journal_ids {
        store.sync_base_journals.remove(id);
    }
    for id in &rev.deleted_template_ids {
        store.sync_base_templates.remove(id);
    }
    if let Some(nickname) = &rev.nickname {
        store.sync_base_nickname = Some(nickname.clone());
    }
    if let Some(resolution_id) = &pending.resolution_id {
        store
            .pending_resolutions
            .retain(|r| r.conflict.id != *resolution_id);
    }
    store
        .pending_uploads
        .retain(|p| p.revision.object_id != rev.object_id);
}

fn upload_revision(
    store: &mut Store,
    transport: &impl RemoteTransport,
    rev: SyncRevision,
    resolution_id: Option<String>,
    summary: &mut SyncSummary,
    save_checkpoint: &mut impl FnMut(&Store) -> Result<(), String>,
) -> Result<(), String> {
    let pending = PendingUpload {
        revision: rev,
        resolution_id,
    };
    store.pending_uploads.push(pending.clone());
    // Persist the exact revision before the network call. A failed final checkpoint
    // can then acknowledge this upload on retry without fabricating a conflict.
    save_checkpoint(store)?;
    let plain = serde_json::to_vec(&pending.revision).map_err(err)?;
    let file_id = transport.put(pending.revision.object_id, &plain)?;
    acknowledge_upload(store, &pending, file_id);
    summary.uploaded_revisions += 1;
    save_checkpoint(store)
}

pub fn sync_with_transport(
    store: &mut Store,
    data_dir: &Path,
    transport: &impl RemoteTransport,
    save_checkpoint: impl FnMut(&Store) -> Result<(), String>,
) -> Result<SyncSummary, String> {
    sync_impl(store, data_dir, transport, save_checkpoint, false)
}
pub fn pull_legacy_with_transport(
    store: &mut Store,
    data_dir: &Path,
    transport: &impl RemoteTransport,
    save_checkpoint: impl FnMut(&Store) -> Result<(), String>,
) -> Result<SyncSummary, String> {
    sync_impl(store, data_dir, transport, save_checkpoint, true)
}
fn sync_impl(
    store: &mut Store,
    data_dir: &Path,
    transport: &impl RemoteTransport,
    mut save_checkpoint: impl FnMut(&Store) -> Result<(), String>,
    pull_only: bool,
) -> Result<SyncSummary, String> {
    if store.device_id.is_empty() {
        store.device_id = Uuid::new_v4().to_string();
    }
    let mut summary = SyncSummary::default();

    if !store.pending_uploads.is_empty() {
        let existing = transport.list()?;
        for pending in store.pending_uploads.clone() {
            let id = pending.revision.object_id;
            let plain = serde_json::to_vec(&pending.revision).map_err(err)?;
            let file_id = if let Some(file) = existing
                .iter()
                .find(|f| parse_revision_uuid(&f.name) == Some(id))
            {
                let remote: SyncRevision = serde_json::from_slice(&transport.get(id, &file.id)?)
                    .map_err(|_| format!("이전 업로드 {}의 형식이 Drive에서 달라졌습니다", id))?;
                if remote != pending.revision {
                    return Err(format!(
                        "이전 업로드 {}의 내용이 Drive에서 달라졌습니다",
                        id
                    ));
                }
                file.id.clone()
            } else {
                let file_id = transport.put(id, &plain)?;
                summary.uploaded_revisions += 1;
                file_id
            };
            acknowledge_upload(store, &pending, file_id);
            save_checkpoint(store)?;
        }
    }

    // 1. Pull remote changes
    let (candidate_files, next_cursor) = if let Some(cursor) = store.sync_cursor.clone()
        && (!store.entries.is_empty() || !store.seen_remote_objects.is_empty())
    {
        let batch = transport.changes(&cursor)?;
        let known_file_ids: HashSet<&str> = store
            .seen_remote_objects
            .values()
            .map(String::as_str)
            .collect();
        let mut removed_known = false;
        let mut files = Vec::new();
        for change in batch.changes {
            if change.removed {
                if known_file_ids.contains(change.file_id.as_str()) {
                    removed_known = true;
                }
            } else if let Some(file) = change.file
                && let Some(obj_id) = parse_revision_uuid(&file.name)
            {
                files.push((file.id, obj_id));
            }
        }
        if removed_known {
            summary.warnings.push(
                "원격 Google Drive에서 이전 동기화 파일이 삭제된 것이 감지되었습니다. 로컬 기록은 유지됩니다."
                    .into(),
            );
        }
        (files, batch.next_cursor)
    } else {
        let start = transport.start_cursor()?;
        let remote_files = transport.list()?;
        if !store.seen_remote_objects.is_empty() {
            let present_ids: HashSet<&str> = remote_files.iter().map(|f| f.id.as_str()).collect();
            if store
                .seen_remote_objects
                .values()
                .any(|fid| !present_ids.contains(fid.as_str()))
            {
                summary.warnings.push(
                    "원격 Google Drive에서 이전 동기화 파일이 누락된 것이 감지되었습니다. 로컬 기록은 유지됩니다."
                        .into(),
                );
            }
        }
        let files = remote_files
            .into_iter()
            .filter_map(|f| parse_revision_uuid(&f.name).map(|id| (f.id, id)))
            .collect();
        (files, start)
    };

    let mut downloaded = Vec::new();
    let mut seen_in_batch = HashSet::new();
    for (file_id, obj_id) in candidate_files {
        let key = obj_id.to_string();
        if store.seen_remote_objects.contains_key(&key)
            || store.ignored_remote_objects.contains(&key)
            || !seen_in_batch.insert(obj_id)
        {
            continue;
        }
        let plain = transport.get(obj_id, &file_id).map_err(|error| {
            if error.contains("Drive 다운로드") || error.contains("Drive 요청") {
                error
            } else {
                format!("BLOCKED_REMOTE:{key}:{error}")
            }
        })?;
        let rev: SyncRevision = serde_json::from_slice(&plain).map_err(|_| {
            format!("BLOCKED_REMOTE:{key}:원격 동기화 버전 형식이 올바르지 않습니다")
        })?;
        if rev.version != 1 || rev.object_id != obj_id {
            return Err(format!(
                "BLOCKED_REMOTE:{key}:원격 동기화 버전 검증에 실패했습니다"
            ));
        }
        if let Some(ref entry) = rev.entry {
            validate_remote_entry(entry).map_err(|e| format!("BLOCKED_REMOTE:{key}:{e}"))?;
        }
        if rev
            .journal
            .as_ref()
            .is_some_and(|j| !valid_remote_journal(j))
            || rev.journals.iter().any(|j| !valid_remote_journal(j))
            || rev.templates.iter().any(|t| !valid_remote_template(t))
            || rev
                .deleted_journal_ids
                .iter()
                .chain(rev.deleted_template_ids.iter())
                .any(|id| id.is_empty() || id.len() > 100)
            || rev
                .nickname
                .as_ref()
                .is_some_and(|name| name.chars().count() > 40)
        {
            return Err(format!(
                "BLOCKED_REMOTE:{key}:원격 동기화 메타데이터 검증에 실패했습니다"
            ));
        }
        downloaded.push((file_id, rev));
    }

    downloaded.sort_by(|a, b| {
        a.1.created_at
            .cmp(&b.1.created_at)
            .then_with(|| a.1.object_id.cmp(&b.1.object_id))
    });

    for (file_id, rev) in &downloaded {
        let rev_id = rev.object_id.to_string();
        store
            .revision_parents
            .insert(rev_id.clone(), rev.parent_ids.clone());
        store.seen_remote_objects.insert(rev_id, file_id.clone());
        summary.downloaded_revisions += 1;
        if let Some(j) = &rev.journal {
            merge_journal(store, &j.id, Some(j.clone()), &mut summary.warnings);
        }
        for j in &rev.journals {
            merge_journal(store, &j.id, Some(j.clone()), &mut summary.warnings);
        }
        for t in &rev.templates {
            merge_template(store, &t.id, Some(t.clone()), &mut summary.warnings);
        }
        for id in &rev.deleted_journal_ids {
            merge_journal(store, id, None, &mut summary.warnings);
        }
        for id in &rev.deleted_template_ids {
            merge_template(store, id, None, &mut summary.warnings);
        }
        if let Some(ref nick) = rev.nickname {
            merge_nickname(store, nick.clone(), &mut summary.warnings);
        }
        for (att_id, manifest) in &rev.media_manifests {
            store
                .media_manifests
                .insert(att_id.clone(), manifest.clone());
        }
    }

    let mut entry_revisions: HashMap<String, Vec<(String, Entry)>> = HashMap::new();
    for (_, rev) in downloaded {
        if let Some(entry) = rev.entry {
            entry_revisions
                .entry(entry.id.clone())
                .or_default()
                .push((rev.object_id.to_string(), entry));
        }
    }

    for (entry_id, revs) in entry_revisions {
        let prev_heads = store
            .remote_heads
            .get(&entry_id)
            .cloned()
            .unwrap_or_default();
        let mut all_ids: Vec<String> = prev_heads.clone();
        for (rev_id, _) in &revs {
            if !all_ids.contains(rev_id) {
                all_ids.push(rev_id.clone());
            }
        }
        let leaf_ids: Vec<String> = all_ids
            .iter()
            .filter(|id| {
                !all_ids
                    .iter()
                    .any(|other| other != *id && is_ancestor(&store.revision_parents, id, other))
            })
            .cloned()
            .collect();

        let mut new_leaves: Vec<(String, Entry)> = revs
            .into_iter()
            .filter(|(id, _)| leaf_ids.contains(id))
            .collect();

        store
            .remote_heads
            .insert(entry_id.clone(), leaf_ids.clone());

        if new_leaves.is_empty() {
            continue;
        }

        for (_, entry) in &mut new_leaves {
            if !store.journals.iter().any(|j| j.id == entry.journal_id) {
                let fallback = Journal {
                    id: entry.journal_id.clone(),
                    name: "동기화된 일지".into(),
                    ..Journal::default()
                };
                store
                    .sync_base_journals
                    .insert(fallback.id.clone(), fallback.clone());
                store.journals.push(fallback);
            }
            restore_entry_media(
                data_dir,
                transport,
                &store.media_manifests,
                entry,
                &mut summary.restored_media,
            )?;
        }

        if new_leaves.len() == 1 && leaf_ids.len() == 1 {
            let (leaf_id, remote_entry) = new_leaves.remove(0);
            let descended_from_prev = prev_heads
                .iter()
                .all(|h| is_ancestor(&store.revision_parents, h, &leaf_id));
            let local_idx = store.entries.iter().position(|e| e.id == entry_id);
            let local = local_idx.map(|i| store.entries[i].clone());

            if store.sync_conflicts.iter().any(|c| c.entry_id == entry_id) {
                upsert_conflict(
                    store,
                    &entry_id,
                    local,
                    conflict_remote_entry(&remote_entry),
                );
                continue;
            }

            let base = if descended_from_prev {
                store.sync_base_entries.get(&entry_id).cloned()
            } else {
                None
            };

            match sync_merge::merge(base.as_ref(), local.as_ref(), Some(&remote_entry)) {
                Decision::Merged(Some(merged)) => {
                    if let Some(i) = local_idx {
                        store.entries[i] = merged.clone();
                    } else {
                        store.entries.push(merged.clone());
                    }
                    store.sync_base_entries.insert(entry_id, remote_entry);
                }
                Decision::Merged(None) => {}
                Decision::Conflict { local, remote } => {
                    let remote_opt = remote.as_ref().and_then(conflict_remote_entry);
                    upsert_conflict(store, &entry_id, local, remote_opt);
                    summary.conflicts_created += 1;
                }
            }
        } else {
            let local_idx = store.entries.iter().position(|e| e.id == entry_id);
            if let Some(i) = local_idx {
                let local_entry = store.entries[i].clone();
                let (_, remote_entry) = new_leaves
                    .into_iter()
                    .find(|(_, e)| *e != local_entry)
                    .unwrap_or_else(|| (String::new(), local_entry.clone()));
                if remote_entry == local_entry {
                    store.sync_base_entries.insert(entry_id, local_entry);
                } else {
                    upsert_conflict(
                        store,
                        &entry_id,
                        Some(local_entry),
                        conflict_remote_entry(&remote_entry),
                    );
                    summary.conflicts_created += 1;
                }
            } else {
                let (_, first) = new_leaves.remove(0);
                store.entries.push(first.clone());
                store
                    .sync_base_entries
                    .insert(entry_id.clone(), first.clone());
                if let Some((_, second)) = new_leaves.into_iter().find(|(_, e)| *e != first) {
                    upsert_conflict(
                        store,
                        &entry_id,
                        Some(first),
                        conflict_remote_entry(&second),
                    );
                    summary.conflicts_created += 1;
                }
            }
        }
    }

    // Commit cursor only after all pulled revisions and media have been applied
    store.sync_cursor = Some(next_cursor);
    save_checkpoint(store)?;
    // Migration may finish explicitly chosen conflict resolutions on legacy
    // devices, but must not upload other new edits with the old key.
    if pull_only && (!store.sync_conflicts.is_empty() || store.pending_resolutions.is_empty()) {
        return Ok(summary);
    }

    // 2. Push pending conflict resolutions
    while let Some(resolution) = store.pending_resolutions.first().cloned() {
        let entry_id = resolution.conflict.entry_id.clone();
        let current = store
            .entries
            .iter()
            .find(|e| e.id == entry_id)
            .cloned()
            .or(resolution.chosen.clone());
        if let Some(entry) = current {
            upload_entry_revision(
                store,
                data_dir,
                transport,
                &entry,
                Some(resolution.conflict.id),
                &mut summary,
                &mut save_checkpoint,
            )?;
        } else {
            return Err("삭제된 기록의 충돌 해결 내용을 업로드할 수 없습니다".into());
        }
    }

    if pull_only {
        return Ok(summary);
    }

    // 3. Push modified local entries (skipping any entry with an unresolved conflict)
    let conflicted_ids: HashSet<String> = store
        .sync_conflicts
        .iter()
        .map(|c| c.entry_id.clone())
        .collect();
    let dirty_entries: Vec<Entry> = store
        .entries
        .iter()
        .filter(|e| {
            !conflicted_ids.contains(&e.id) && store.sync_base_entries.get(&e.id) != Some(*e)
        })
        .cloned()
        .collect();

    for entry in dirty_entries {
        upload_entry_revision(
            store,
            data_dir,
            transport,
            &entry,
            None,
            &mut summary,
            &mut save_checkpoint,
        )?;
    }

    // 4. Push modified journals, templates, and nickname
    let dirty_journals: Vec<Journal> = store
        .journals
        .iter()
        .filter(|j| store.sync_base_journals.get(&j.id) != Some(*j))
        .cloned()
        .collect();
    let dirty_templates: Vec<Template> = store
        .templates
        .iter()
        .filter(|t| store.sync_base_templates.get(&t.id) != Some(*t))
        .cloned()
        .collect();
    let deleted_journal_ids: Vec<String> = store
        .sync_base_journals
        .keys()
        .filter(|id| !store.journals.iter().any(|j| &j.id == *id))
        .cloned()
        .collect();
    let deleted_template_ids: Vec<String> = store
        .sync_base_templates
        .keys()
        .filter(|id| !store.templates.iter().any(|t| &t.id == *id))
        .cloned()
        .collect();
    let dirty_nickname = if store.sync_base_nickname.as_deref() != Some(store.nickname.as_str()) {
        Some(store.nickname.clone())
    } else {
        None
    };

    if !dirty_journals.is_empty()
        || !dirty_templates.is_empty()
        || !deleted_journal_ids.is_empty()
        || !deleted_template_ids.is_empty()
        || dirty_nickname.is_some()
    {
        let object_id = new_revision_uuid();
        let rev = SyncRevision {
            version: 1,
            object_id,
            parent_ids: Vec::new(),
            device_id: store.device_id.clone(),
            created_at: now(),
            entry: None,
            journal: None,
            journals: dirty_journals.clone(),
            templates: dirty_templates.clone(),
            deleted_journal_ids,
            deleted_template_ids,
            nickname: dirty_nickname.clone(),
            media_manifests: HashMap::new(),
        };
        upload_revision(
            store,
            transport,
            rev,
            None,
            &mut summary,
            &mut save_checkpoint,
        )?;
    }

    store.last_sync_at = Some(now());
    store.last_sync_error = None;
    store.blocked_remote_object = None;
    save_checkpoint(store)?;
    Ok(summary)
}

fn upload_entry_revision(
    store: &mut Store,
    data_dir: &Path,
    transport: &impl RemoteTransport,
    entry: &Entry,
    resolution_id: Option<String>,
    summary: &mut SyncSummary,
    save_checkpoint: &mut impl FnMut(&Store) -> Result<(), String>,
) -> Result<(), String> {
    let mut entry_manifests = HashMap::new();
    for attachment in &entry.attachments {
        let id = Uuid::parse_str(&attachment.id).map_err(|_| "잘못된 첨부 식별자")?;
        let needs_upload = store
            .media_manifests
            .get(&attachment.id)
            .is_none_or(|m| m.bytes != attachment.bytes);
        let manifest = if needs_upload {
            let path = data_dir.join("media").join(id.to_string());
            let bytes = fs::read(path)
                .map_err(|_| format!("첨부 파일을 읽을 수 없습니다: {}", attachment.name))?;
            let uploaded = sync_media::upload(transport, &bytes)?;
            store
                .media_manifests
                .insert(attachment.id.clone(), uploaded.clone());
            summary.uploaded_media += 1;
            uploaded
        } else {
            store.media_manifests[&attachment.id].clone()
        };
        entry_manifests.insert(attachment.id.clone(), manifest);
    }

    let object_id = new_revision_uuid();
    let parent_ids = store
        .remote_heads
        .get(&entry.id)
        .cloned()
        .unwrap_or_default();
    let journal = store
        .journals
        .iter()
        .find(|j| j.id == entry.journal_id)
        .cloned();
    let rev = SyncRevision {
        version: 1,
        object_id,
        parent_ids: parent_ids.clone(),
        device_id: store.device_id.clone(),
        created_at: now(),
        entry: Some(entry.clone()),
        journal: journal.clone(),
        journals: Vec::new(),
        templates: Vec::new(),
        deleted_journal_ids: Vec::new(),
        deleted_template_ids: Vec::new(),
        nickname: None,
        media_manifests: entry_manifests,
    };
    upload_revision(
        store,
        transport,
        rev,
        resolution_id,
        summary,
        save_checkpoint,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Attachment,
        drive_transport::{ChangeBatch, DriveChange, DriveFile},
        resolve_conflict, sync_crypto,
        sync_media::BlobStore,
    };
    use std::cell::RefCell;

    #[derive(Default)]
    struct MockCloud {
        files: Vec<(String, String, Vec<u8>)>, // (file_id, name, ciphertext)
        changes: Vec<DriveChange>,
        next_id: usize,
    }

    struct MockClient<'a> {
        cloud: &'a RefCell<MockCloud>,
        key: [u8; 32],
    }

    impl BlobStore for MockClient<'_> {
        fn put(&self, object_id: Uuid, bytes: &[u8]) -> Result<String, String> {
            let ciphertext =
                sync_crypto::encrypt(&self.key, object_id.to_string().as_bytes(), bytes)?;
            let mut cloud = self.cloud.borrow_mut();
            cloud.next_id += 1;
            let file_id = format!("file_{}", cloud.next_id);
            let name = object_id.to_string();
            cloud
                .files
                .push((file_id.clone(), name.clone(), ciphertext));
            cloud.changes.push(DriveChange {
                file_id: file_id.clone(),
                removed: false,
                file: Some(DriveFile {
                    id: file_id.clone(),
                    name,
                }),
            });
            Ok(file_id)
        }

        fn get(&self, object_id: Uuid, file_id: &str) -> Result<Vec<u8>, String> {
            let cloud = self.cloud.borrow();
            let (_, _, ciphertext) = cloud
                .files
                .iter()
                .find(|(id, _, _)| id == file_id)
                .ok_or_else(|| "원격 파일 없음".to_string())?;
            sync_crypto::decrypt(&self.key, object_id.to_string().as_bytes(), ciphertext)
                .map_err(str::to_string)
        }
    }

    impl RemoteTransport for MockClient<'_> {
        fn list(&self) -> Result<Vec<DriveFile>, String> {
            Ok(self
                .cloud
                .borrow()
                .files
                .iter()
                .map(|(id, name, _)| DriveFile {
                    id: id.clone(),
                    name: name.clone(),
                })
                .collect())
        }

        fn start_cursor(&self) -> Result<String, String> {
            Ok(self.cloud.borrow().changes.len().to_string())
        }

        fn changes(&self, cursor: &str) -> Result<ChangeBatch, String> {
            let idx: usize = cursor.parse().map_err(|_| "잘못된 커서")?;
            let cloud = self.cloud.borrow();
            let slice = if idx <= cloud.changes.len() {
                cloud.changes[idx..].to_vec()
            } else {
                Vec::new()
            };
            Ok(ChangeBatch {
                changes: slice,
                next_cursor: cloud.changes.len().to_string(),
            })
        }
    }

    struct ScopedMock<'a>(MockClient<'a>);
    impl BlobStore for ScopedMock<'_> {
        fn put(&self, id: Uuid, bytes: &[u8]) -> Result<String, String> {
            let file_id = self.0.put(id, bytes)?;
            let mut cloud = self.0.cloud.borrow_mut();
            let name = crate::drive_transport::v2_name(id);
            cloud.files.last_mut().unwrap().1 = name.clone();
            cloud
                .changes
                .last_mut()
                .unwrap()
                .file
                .as_mut()
                .unwrap()
                .name = name;
            Ok(file_id)
        }
        fn get(&self, id: Uuid, file_id: &str) -> Result<Vec<u8>, String> {
            self.0.get(id, file_id)
        }
    }
    impl RemoteTransport for ScopedMock<'_> {
        fn list(&self) -> Result<Vec<DriveFile>, String> {
            Ok(self
                .0
                .list()?
                .into_iter()
                .filter_map(|mut f| {
                    f.name = crate::drive_transport::decode_v2_name(&f.name)?;
                    Some(f)
                })
                .collect())
        }
        fn start_cursor(&self) -> Result<String, String> {
            self.0.start_cursor()
        }
        fn changes(&self, cursor: &str) -> Result<ChangeBatch, String> {
            let mut batch = self.0.changes(cursor)?;
            for change in &mut batch.changes {
                if let Some(f) = &mut change.file {
                    f.name = crate::drive_transport::decode_v2_name(&f.name).unwrap_or_default();
                }
            }
            Ok(batch)
        }
    }

    #[test]
    fn migrated_entries_and_media_restore_on_another_device_without_touching_legacy() {
        let cloud = RefCell::new(MockCloud::default());
        let legacy = MockClient {
            cloud: &cloud,
            key: crate::drive_transport::derive_account_sync_key("test@example.com"),
        };
        let shared_key = sync_crypto::generate_key().unwrap();
        let v2 = ScopedMock(MockClient {
            cloud: &cloud,
            key: shared_key,
        });
        let dir_a = temp_device_dir("migration-a");
        let dir_b = temp_device_dir("migration-b");
        let journal = Journal::default();
        let attachment_id = Uuid::new_v4().to_string();
        let bytes = b"\x89PNG\r\n\x1a\nimage";
        fs::write(dir_a.join("media").join(&attachment_id), bytes).unwrap();
        let entry = Entry {
            journal_id: journal.id.clone(),
            body: "**기존 기록**".into(),
            attachments: vec![Attachment {
                id: attachment_id.clone(),
                name: "photo.png".into(),
                mime: "image/png".into(),
                bytes: bytes.len(),
            }],
            ..Entry::default()
        };
        let mut a = Store {
            journals: vec![journal],
            entries: vec![entry.clone()],
            ..Store::default()
        };
        sync_with_transport(&mut a, &dir_a, &legacy, |_| Ok(())).unwrap();
        let legacy_count = cloud.borrow().files.len();
        a.entries[0].body = "**이전 후 새 편집**".into();
        crate::prepare_v2_migration(
            &mut a,
            &dir_a,
            &legacy,
            "test@example.com",
            || Ok(()),
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(cloud.borrow().files.len(), legacy_count);
        assert_eq!(a.sync_format, 2);
        assert_eq!(a.sync_account.as_deref(), Some("test@example.com"));
        sync_with_transport(&mut a, &dir_a, &v2, |_| Ok(())).unwrap();
        assert!(cloud.borrow().files.len() > legacy_count);
        assert!(
            cloud
                .borrow()
                .files
                .iter()
                .skip(legacy_count)
                .all(|(_, name, _)| crate::sync_engine::parse_revision_uuid(name).is_none())
        );
        let mut b = Store::default();
        sync_with_transport(&mut b, &dir_b, &v2, |_| Ok(())).unwrap();
        assert_eq!(b.entries[0].body, "**이전 후 새 편집**");
        assert_eq!(
            fs::read(dir_b.join("media").join(attachment_id)).unwrap(),
            bytes
        );
        assert_eq!(
            cloud.borrow().files.iter().take(legacy_count).count(),
            legacy_count
        );
        fs::remove_dir_all(dir_a).unwrap();
        fs::remove_dir_all(dir_b).unwrap();
    }

    fn temp_device_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::current_dir()
            .unwrap()
            .join("dev-data")
            .join(format!("sync-test-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(dir.join("media")).unwrap();
        dir
    }

    #[test]
    fn migration_does_not_switch_keys_while_conflicts_remain() {
        let cloud = RefCell::new(MockCloud::default());
        let legacy = MockClient {
            cloud: &cloud,
            key: [3u8; 32],
        };
        let dir = temp_device_dir("migration-conflict");
        let mut store = Store {
            sync_conflicts: vec![SyncConflict {
                id: "conflict".into(),
                entry_id: "entry".into(),
                local: None,
                remote: None,
            }],
            ..Store::default()
        };
        let called = std::cell::Cell::new(false);
        let result = crate::prepare_v2_migration(
            &mut store,
            &dir,
            &legacy,
            "account",
            || {
                called.set(true);
                Ok(())
            },
            |_| Ok(()),
        );
        assert!(result.is_err());
        assert!(!called.get());
        assert_eq!(store.sync_format, 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn concurrent_local_edit_is_preserved_and_conflicts_on_retry() {
        let cloud = RefCell::new(MockCloud::default());
        let client = MockClient {
            cloud: &cloud,
            key: sync_crypto::generate_key().unwrap(),
        };
        let dir_a = temp_device_dir("race-a");
        let dir_b = temp_device_dir("race-b");
        let journal = Journal::default();
        let entry = Entry {
            journal_id: journal.id.clone(),
            body: "original".into(),
            ..Entry::default()
        };
        let mut a = Store {
            journals: vec![journal],
            entries: vec![entry],
            ..Store::default()
        };
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        let mut b = Store::default();
        sync_with_transport(&mut b, &dir_b, &client, |_| Ok(())).unwrap();
        a.entries[0].body = "remote edit".into();
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();

        let live = std::sync::Mutex::new(b.clone());
        let mut base = serde_json::to_value(&b).unwrap();
        let mut edited = false;
        let result = sync_with_transport(&mut b, &dir_b, &client, |checkpoint| {
            if !edited {
                edited = true;
                let mut store = live.lock().unwrap();
                store.entries[0].body = "local edit".into();
                fs::write(
                    dir_b.join("journal.json"),
                    serde_json::to_vec(&*store).unwrap(),
                )
                .unwrap();
            }
            crate::commit_sync_checkpoint(&live, &dir_b, &mut base, checkpoint)
        });
        assert!(result.is_err());
        let persisted: Store =
            serde_json::from_slice(&fs::read(dir_b.join("journal.json")).unwrap()).unwrap();
        assert_eq!(persisted.entries[0].body, "local edit");
        let mut retry = live.into_inner().unwrap();
        sync_with_transport(&mut retry, &dir_b, &client, |_| Ok(())).unwrap();
        assert_eq!(retry.entries[0].body, "local edit");
        assert_eq!(retry.sync_conflicts.len(), 1);
        assert_eq!(
            retry.sync_conflicts[0].remote.as_ref().unwrap().body,
            "remote edit"
        );
        fs::remove_dir_all(dir_a).unwrap();
        fs::remove_dir_all(dir_b).unwrap();
    }

    #[test]
    fn syncs_entries_media_journals_templates_and_restores_on_new_device() {
        let cloud = RefCell::new(MockCloud::default());
        let key = sync_crypto::generate_key().unwrap();
        let client = MockClient { cloud: &cloud, key };

        let dir_a = temp_device_dir("a");
        let dir_b = temp_device_dir("b");
        let dir_c = temp_device_dir("c");

        let journal_a = Journal {
            id: "journal-main".into(),
            name: "일상 기록".into(),
            color: "#3b6e5c".into(),
            created_at: "2026-10-29T00:00:00Z".into(),
        };
        let png_bytes = b"\x89PNG\r\n\x1a\nfake-png-content".to_vec();
        let att_id = Uuid::new_v4().to_string();
        fs::write(dir_a.join("media").join(&att_id), &png_bytes).unwrap();

        let entry_a = Entry {
            id: "entry-1".into(),
            journal_id: journal_a.id.clone(),
            title: "첫 번째 일기".into(),
            body: format!("## 오늘\n**사진**과 함께 남긴 기록\n\n![photo.png](niki-media:{att_id})\n\n- [x] 산책\n- [ ] 독서"),
            date: "2026-10-29".into(),
            tags: vec!["여행".into()],
            attachments: vec![Attachment {
                id: att_id.clone(),
                name: "photo.png".into(),
                mime: "image/png".into(),
                bytes: png_bytes.len(),
            }],
            ..Entry::default()
        };
        let template_a = Template {
            id: "tpl-1".into(),
            name: "감사 일기".into(),
            body: "## 오늘 감사한 일\n- ".into(),
        };

        let mut store_a = Store {
            version: 1,
            journals: vec![journal_a.clone()],
            entries: vec![entry_a.clone()],
            templates: vec![template_a.clone()],
            ..Store::default()
        };

        let entry_b = Entry {
            id: "entry-2".into(),
            journal_id: journal_a.id.clone(),
            title: "두 번째 기기 일기".into(),
            body: "다른 PC에서 작성".into(),
            date: "2026-10-29".into(),
            ..Entry::default()
        };
        let mut store_b = Store {
            version: 1,
            journals: vec![journal_a.clone()],
            entries: vec![entry_b.clone()],
            ..Store::default()
        };

        // Device A syncs (uploads entry-1 + media + template)
        let sum_a1 = sync_with_transport(&mut store_a, &dir_a, &client, |_| Ok(())).unwrap();
        assert_eq!(sum_a1.uploaded_media, 1);
        assert!(sum_a1.uploaded_revisions >= 1);

        // Device B syncs (downloads entry-1 + media + template, uploads entry-2)
        let sum_b1 = sync_with_transport(&mut store_b, &dir_b, &client, |_| Ok(())).unwrap();
        assert_eq!(sum_b1.restored_media, 1);
        assert_eq!(store_b.entries.len(), 2);
        assert_eq!(
            store_b
                .entries
                .iter()
                .find(|e| e.id == entry_a.id)
                .unwrap()
                .body,
            entry_a.body
        );
        assert_eq!(store_b.templates.len(), 1);
        assert_eq!(
            fs::read(dir_b.join("media").join(&att_id)).unwrap(),
            png_bytes
        );

        // Device A syncs again (downloads entry-2 via changes cursor)
        let sum_a2 = sync_with_transport(&mut store_a, &dir_a, &client, |_| Ok(())).unwrap();
        assert_eq!(sum_a2.downloaded_revisions, 1);
        assert_eq!(store_a.entries.len(), 2);

        // Idempotent re-sync produces 0 uploads and 0 duplicates
        let sum_a3 = sync_with_transport(&mut store_a, &dir_a, &client, |_| Ok(())).unwrap();
        assert_eq!(sum_a3.uploaded_revisions, 0);
        assert_eq!(sum_a3.downloaded_revisions, 0);
        assert_eq!(store_a.entries.len(), 2);

        // Device C (brand new PC with recovery key) restores everything
        let mut store_c = Store::default();
        let sum_c = sync_with_transport(&mut store_c, &dir_c, &client, |_| Ok(())).unwrap();
        assert_eq!(sum_c.restored_media, 1);
        assert_eq!(store_c.entries.len(), 2);
        assert_eq!(
            store_c
                .entries
                .iter()
                .find(|e| e.id == entry_a.id)
                .unwrap()
                .body,
            entry_a.body
        );
        assert_eq!(store_c.templates.len(), 1);
        assert_eq!(
            fs::read(dir_c.join("media").join(&att_id)).unwrap(),
            png_bytes
        );

        // Wrong recovery key cannot decrypt anything
        let wrong_client = MockClient {
            cloud: &cloud,
            key: [99u8; 32],
        };
        let mut store_wrong = Store::default();
        assert!(sync_with_transport(&mut store_wrong, &dir_c, &wrong_client, |_| Ok(())).is_err());

        let _ = fs::remove_dir_all(dir_a);
        let _ = fs::remove_dir_all(dir_b);
        let _ = fs::remove_dir_all(dir_c);
    }

    #[test]
    fn preserves_concurrent_edit_conflicts_and_propagates_resolution() {
        let cloud = RefCell::new(MockCloud::default());
        let key = sync_crypto::generate_key().unwrap();
        let client = MockClient { cloud: &cloud, key };

        let dir_a = temp_device_dir("conf-a");
        let dir_b = temp_device_dir("conf-b");

        let journal = Journal::default();
        let base_entry = Entry {
            id: "shared-entry".into(),
            journal_id: journal.id.clone(),
            title: "공통 제목".into(),
            body: "초기 본문".into(),
            date: "2026-10-29".into(),
            ..Entry::default()
        };

        let mut store_a = Store {
            version: 1,
            journals: vec![journal.clone()],
            entries: vec![base_entry],
            ..Store::default()
        };
        let mut store_b = Store {
            version: 1,
            journals: vec![journal],
            ..Store::default()
        };

        // Initial sync between A and B
        sync_with_transport(&mut store_a, &dir_a, &client, |_| Ok(())).unwrap();
        sync_with_transport(&mut store_b, &dir_b, &client, |_| Ok(())).unwrap();
        assert_eq!(store_b.entries.len(), 1);

        // Both A and B edit the same entry differently offline
        store_a.entries[0].body = "기기 A에서 수정한 내용".into();
        store_b.entries[0].body = "기기 B에서 수정한 내용".into();

        // A syncs first
        sync_with_transport(&mut store_a, &dir_a, &client, |_| Ok(())).unwrap();

        // B syncs and detects conflict without losing either version
        let sum_b = sync_with_transport(&mut store_b, &dir_b, &client, |_| Ok(())).unwrap();
        assert_eq!(sum_b.conflicts_created, 1);
        assert_eq!(store_b.sync_conflicts.len(), 1);
        assert_eq!(store_b.entries[0].body, "기기 B에서 수정한 내용");
        assert_eq!(
            store_b.sync_conflicts[0].remote.as_ref().unwrap().body,
            "기기 A에서 수정한 내용"
        );

        // User on B resolves conflict by choosing "remote" (or "local")
        let conflict_id = store_b.sync_conflicts[0].id.clone();
        resolve_conflict(&mut store_b, &dir_b, &conflict_id, "local").unwrap();
        assert!(store_b.sync_conflicts.is_empty());
        assert_eq!(store_b.pending_resolutions.len(), 1);

        // B syncs the resolution; A syncs and converges to B's chosen version
        sync_with_transport(&mut store_b, &dir_b, &client, |_| Ok(())).unwrap();
        assert!(store_b.pending_resolutions.is_empty());

        sync_with_transport(&mut store_a, &dir_a, &client, |_| Ok(())).unwrap();
        assert!(store_a.sync_conflicts.is_empty());
        assert_eq!(store_a.entries[0].body, "기기 B에서 수정한 내용");

        let _ = fs::remove_dir_all(dir_a);
        let _ = fs::remove_dir_all(dir_b);
    }

    #[test]
    fn detects_deleted_or_tampered_remote_revisions() {
        let cloud = RefCell::new(MockCloud::default());
        let key = sync_crypto::generate_key().unwrap();
        let client = MockClient { cloud: &cloud, key };

        let dir_a = temp_device_dir("tamper-a");
        let dir_b = temp_device_dir("tamper-b");

        let journal = Journal::default();
        let mut store_a = Store {
            version: 1,
            journals: vec![journal.clone()],
            entries: vec![Entry {
                id: "e1".into(),
                journal_id: journal.id.clone(),
                body: "원본".into(),
                ..Entry::default()
            }],
            ..Store::default()
        };
        sync_with_transport(&mut store_a, &dir_a, &client, |_| Ok(())).unwrap();

        // Simulate remote deletion notification in changes feed
        let removed_file_id = cloud.borrow().files[0].0.clone();
        cloud.borrow_mut().changes.push(DriveChange {
            file_id: removed_file_id,
            removed: true,
            file: None,
        });
        let sum = sync_with_transport(&mut store_a, &dir_a, &client, |_| Ok(())).unwrap();
        assert_eq!(sum.warnings.len(), 1);
        assert_eq!(store_a.entries[0].body, "원본");

        // Corrupt remote ciphertext before Device B syncs
        cloud.borrow_mut().files[0].2.push(0xff);
        let mut store_b = Store::default();
        assert!(sync_with_transport(&mut store_b, &dir_b, &client, |_| Ok(())).is_err());
        assert!(store_b.entries.is_empty());

        let _ = fs::remove_dir_all(dir_a);
        let _ = fs::remove_dir_all(dir_b);
    }

    #[test]
    fn uploaded_edit_is_acknowledged_after_a_concurrent_local_save() {
        let cloud = RefCell::new(MockCloud::default());
        let client = MockClient {
            cloud: &cloud,
            key: sync_crypto::generate_key().unwrap(),
        };
        let dir_a = temp_device_dir("ack-race-a");
        let dir_b = temp_device_dir("ack-race-b");
        let journal = Journal::default();
        let mut a = Store {
            journals: vec![journal.clone()],
            entries: vec![Entry {
                journal_id: journal.id.clone(),
                body: "first".into(),
                ..Entry::default()
            }],
            ..Store::default()
        };
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        let mut b = Store::default();
        sync_with_transport(&mut b, &dir_b, &client, |_| Ok(())).unwrap();
        a.entries[0].body = "uploaded".into();
        let live = std::sync::Mutex::new(a.clone());
        let mut base = serde_json::to_value(&a).unwrap();
        let known = a.seen_remote_objects.len();
        let mut edited = false;
        let result = sync_with_transport(&mut a, &dir_a, &client, |checkpoint| {
            if !edited && checkpoint.seen_remote_objects.len() > known {
                edited = true;
                let mut live_store = live.lock().unwrap();
                live_store.entries[0].body = "newer local save".into();
                fs::write(
                    dir_a.join("journal.json"),
                    serde_json::to_vec(&*live_store).unwrap(),
                )
                .unwrap();
            }
            crate::commit_sync_checkpoint(&live, &dir_a, &mut base, checkpoint)
        });
        assert!(result.is_err());
        let mut retry = live.into_inner().unwrap();
        assert_eq!(retry.pending_uploads.len(), 1);
        sync_with_transport(&mut retry, &dir_a, &client, |_| Ok(())).unwrap();
        assert!(retry.pending_uploads.is_empty());
        assert!(retry.sync_conflicts.is_empty());
        assert_eq!(retry.entries[0].body, "newer local save");
        sync_with_transport(&mut b, &dir_b, &client, |_| Ok(())).unwrap();
        assert_eq!(b.entries[0].body, "newer local save");
        assert!(b.sync_conflicts.is_empty());
        fs::remove_dir_all(dir_a).unwrap();
        fs::remove_dir_all(dir_b).unwrap();
    }

    #[test]
    fn nickname_clear_and_offline_rename_converge_without_losing_local_edit() {
        let cloud = RefCell::new(MockCloud::default());
        let client = MockClient {
            cloud: &cloud,
            key: sync_crypto::generate_key().unwrap(),
        };
        let dir_a = temp_device_dir("nick-a");
        let dir_b = temp_device_dir("nick-b");
        let mut a = Store {
            nickname: "첫 별명".into(),
            ..Store::default()
        };
        let mut b = Store::default();
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        sync_with_transport(&mut b, &dir_b, &client, |_| Ok(())).unwrap();
        assert_eq!(b.nickname, "첫 별명");
        a.nickname.clear();
        assert!(pending_upload_count(&a) > 0);
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        sync_with_transport(&mut b, &dir_b, &client, |_| Ok(())).unwrap();
        assert_eq!(b.nickname, "");
        a.nickname = "기기 A".into();
        b.nickname = "기기 B".into();
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        let summary = sync_with_transport(&mut b, &dir_b, &client, |_| Ok(())).unwrap();
        assert!(!summary.warnings.is_empty());
        assert_eq!(b.nickname, "기기 B");
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        assert_eq!(a.nickname, "기기 B");
        fs::remove_dir_all(dir_a).unwrap();
        fs::remove_dir_all(dir_b).unwrap();
    }

    #[test]
    fn metadata_deletion_propagates_and_concurrent_edits_keep_copies() {
        let cloud = RefCell::new(MockCloud::default());
        let client = MockClient {
            cloud: &cloud,
            key: sync_crypto::generate_key().unwrap(),
        };
        let dir_a = temp_device_dir("metadata-a");
        let dir_b = temp_device_dir("metadata-b");
        let journal = Journal {
            name: "첫 일지".into(),
            ..Journal::default()
        };
        let template = Template {
            name: "첫 서식".into(),
            ..Template::default()
        };
        let mut a = Store {
            journals: vec![journal.clone()],
            templates: vec![template.clone()],
            ..Store::default()
        };
        let mut b = Store::default();
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        sync_with_transport(&mut b, &dir_b, &client, |_| Ok(())).unwrap();
        a.journals.clear();
        a.templates.clear();
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        sync_with_transport(&mut b, &dir_b, &client, |_| Ok(())).unwrap();
        assert!(b.journals.is_empty() && b.templates.is_empty());
        a.journals.push(journal);
        a.templates.push(template);
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        sync_with_transport(&mut b, &dir_b, &client, |_| Ok(())).unwrap();
        a.journals[0].name = "A 일지".into();
        a.templates[0].name = "A 서식".into();
        b.journals[0].name = "B 일지".into();
        b.templates[0].name = "B 서식".into();
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        let summary = sync_with_transport(&mut b, &dir_b, &client, |_| Ok(())).unwrap();
        assert_eq!(summary.warnings.len(), 2);
        assert!(b.journals.iter().any(|j| j.name == "A 일지"));
        assert!(b.journals.iter().any(|j| j.name.starts_with("B 일지")));
        assert!(b.templates.iter().any(|t| t.name == "A 서식"));
        assert!(b.templates.iter().any(|t| t.name.starts_with("B 서식")));
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        assert_eq!(a.journals.len(), 2);
        assert_eq!(a.templates.len(), 2);
        fs::remove_dir_all(dir_a).unwrap();
        fs::remove_dir_all(dir_b).unwrap();
    }

    #[test]
    fn malformed_revision_can_be_explicitly_skipped_without_discarding_valid_revisions() {
        let cloud = RefCell::new(MockCloud::default());
        let client = MockClient {
            cloud: &cloud,
            key: sync_crypto::generate_key().unwrap(),
        };
        let dir = temp_device_dir("bad-revision");
        let bad_id = new_revision_uuid();
        client.put(bad_id, b"bad json").unwrap();
        let mut store = Store::default();
        let error = sync_with_transport(&mut store, &dir, &client, |_| Ok(())).unwrap_err();
        assert!(error.starts_with(&format!("BLOCKED_REMOTE:{bad_id}:")));
        store.ignored_remote_objects.insert(bad_id.to_string());
        sync_with_transport(&mut store, &dir, &client, |_| Ok(())).unwrap();
        assert!(store.sync_cursor.is_some());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_checkpoint_keeps_all_conflict_resolutions_for_retry() {
        let cloud = RefCell::new(MockCloud::default());
        let client = MockClient {
            cloud: &cloud,
            key: sync_crypto::generate_key().unwrap(),
        };
        let dir_a = temp_device_dir("resolution-a");
        let dir_b = temp_device_dir("resolution-b");
        let journal = Journal::default();
        let entries: Vec<Entry> = (0..2)
            .map(|i| Entry {
                journal_id: journal.id.clone(),
                body: format!("base {i}"),
                ..Entry::default()
            })
            .collect();
        let mut a = Store {
            journals: vec![journal],
            entries,
            ..Store::default()
        };
        let mut b = Store::default();
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        sync_with_transport(&mut b, &dir_b, &client, |_| Ok(())).unwrap();
        for entry in &mut a.entries {
            entry.body = "A".into();
        }
        for entry in &mut b.entries {
            entry.body = "B".into();
        }
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        sync_with_transport(&mut b, &dir_b, &client, |_| Ok(())).unwrap();
        assert_eq!(b.sync_conflicts.len(), 2);
        for id in b
            .sync_conflicts
            .iter()
            .map(|c| c.id.clone())
            .collect::<Vec<_>>()
        {
            resolve_conflict(&mut b, &dir_b, &id, "local").unwrap();
        }
        let saved = RefCell::new(b.clone());
        let mut fail = false;
        let result = sync_with_transport(&mut b, &dir_b, &client, |checkpoint| {
            if !fail
                && checkpoint.pending_resolutions.len() == 1
                && checkpoint.pending_uploads.is_empty()
            {
                fail = true;
                return Err("simulated checkpoint failure".into());
            }
            *saved.borrow_mut() = checkpoint.clone();
            Ok(())
        });
        assert!(result.is_err());
        let mut retry = saved.into_inner();
        assert_eq!(retry.pending_resolutions.len(), 2);
        assert_eq!(retry.pending_uploads.len(), 1);
        sync_with_transport(&mut retry, &dir_b, &client, |_| Ok(())).unwrap();
        assert!(retry.pending_resolutions.is_empty());
        assert!(retry.pending_uploads.is_empty());
        sync_with_transport(&mut a, &dir_a, &client, |_| Ok(())).unwrap();
        assert!(a.sync_conflicts.is_empty());
        assert!(a.entries.iter().all(|e| e.body == "B"));
        fs::remove_dir_all(dir_a).unwrap();
        fs::remove_dir_all(dir_b).unwrap();
    }

    #[test]
    fn earlier_store_and_revision_json_remain_readable() {
        let store = Store::default();
        let mut old_store = serde_json::to_value(&store).unwrap();
        let fields = old_store.as_object_mut().unwrap();
        for name in [
            "pending_uploads",
            "ignored_remote_objects",
            "blocked_remote_object",
        ] {
            fields.remove(name);
        }
        let restored: Store = serde_json::from_value(old_store).unwrap();
        assert!(restored.pending_uploads.is_empty());
        assert!(restored.ignored_remote_objects.is_empty());
        let old_revision = serde_json::json!({
            "version": 1,
            "object_id": new_revision_uuid(),
            "parent_ids": [],
            "device_id": "old-device",
            "created_at": "2026-10-29T00:00:00Z",
            "entry": null,
            "journal": null
        });
        let restored: SyncRevision = serde_json::from_value(old_revision).unwrap();
        assert!(restored.deleted_journal_ids.is_empty());
        assert!(restored.deleted_template_ids.is_empty());
    }
}
