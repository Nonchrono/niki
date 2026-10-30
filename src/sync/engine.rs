//! Encrypted Google Drive synchronization engine.

use crate::{
    Entry, Journal, Store, SyncConflict, Template,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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
            !conflicted.contains(e.id.as_str())
                && store.sync_base_entries.get(&e.id) != Some(*e)
        })
        .count();
    let journal_changes = store
        .journals
        .iter()
        .filter(|j| store.sync_base_journals.get(&j.id) != Some(*j))
        .count();
    let template_changes = store
        .templates
        .iter()
        .filter(|t| store.sync_base_templates.get(&t.id) != Some(*t))
        .count();
    let nickname_changes = if store.sync_base_nickname.as_deref() != Some(store.nickname.as_str())
        && !store.nickname.is_empty()
    {
        1
    } else {
        0
    };
    entry_changes + journal_changes + template_changes + nickname_changes + store.pending_resolutions.len()
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

fn merge_journal(store: &mut Store, mut remote: Journal) {
    let name = remote.name.trim().to_string();
    if remote.id.is_empty() || remote.id.len() > 100 || name.is_empty() || name.chars().count() > 80
    {
        return;
    }
    remote.name = name;
    if !valid_color(&remote.color) {
        remote.color = "#576b61".into();
    }
    let base = store.sync_base_journals.get(&remote.id);
    let local_idx = store.journals.iter().position(|j| j.id == remote.id);
    let local = local_idx.map(|i| &store.journals[i]);
    match sync_merge::merge(base, local, Some(&remote)) {
        Decision::Merged(Some(merged)) => {
            if let Some(i) = local_idx {
                store.journals[i] = merged.clone();
            } else {
                store.journals.push(merged.clone());
            }
            store.sync_base_journals.insert(remote.id, merged);
        }
        Decision::Conflict {
            local: Some(local_j),
            remote: Some(remote_j),
        } => {
            store.sync_base_journals.insert(remote_j.id, local_j);
        }
        _ => {}
    }
}

fn merge_template(store: &mut Store, remote: Template) {
    if remote.id.is_empty()
        || remote.id.len() > 100
        || remote.name.trim().is_empty()
        || remote.name.len() > 500
        || remote.body.len() > 2_000_000
    {
        return;
    }
    let base = store.sync_base_templates.get(&remote.id);
    let local_idx = store.templates.iter().position(|t| t.id == remote.id);
    let local = local_idx.map(|i| &store.templates[i]);
    match sync_merge::merge(base, local, Some(&remote)) {
        Decision::Merged(Some(merged)) => {
            if let Some(i) = local_idx {
                store.templates[i] = merged.clone();
            } else {
                store.templates.push(merged.clone());
            }
            store.sync_base_templates.insert(remote.id, merged);
        }
        Decision::Conflict {
            local: Some(local_t),
            remote: Some(remote_t),
        } => {
            store.sync_base_templates.insert(remote_t.id, local_t);
        }
        _ => {}
    }
}

fn merge_nickname(store: &mut Store, remote: String) {
    let remote = remote.trim();
    if remote.chars().count() > 40 {
        return;
    }
    let base = store.sync_base_nickname.as_deref();
    let local = store.nickname.as_str();
    if base == Some(local) || local.is_empty() || local != remote {
        store.nickname = remote.to_string();
        store.sync_base_nickname = Some(remote.to_string());
    } else {
        store.sync_base_nickname = Some(remote.to_string());
    }
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

pub fn sync_with_transport(
    store: &mut Store,
    data_dir: &Path,
    transport: &impl RemoteTransport,
    save_checkpoint: impl FnMut(&Store) -> Result<(), String>,
) -> Result<SyncSummary, String> {
    sync_impl(store,data_dir,transport,save_checkpoint,false)
}
pub fn pull_legacy_with_transport(
    store: &mut Store,
    data_dir: &Path,
    transport: &impl RemoteTransport,
    save_checkpoint: impl FnMut(&Store) -> Result<(), String>,
) -> Result<SyncSummary, String> {
    sync_impl(store,data_dir,transport,save_checkpoint,true)
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
            let present_ids: HashSet<&str> =
                remote_files.iter().map(|f| f.id.as_str()).collect();
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
        if store.seen_remote_objects.contains_key(&key) || !seen_in_batch.insert(obj_id) {
            continue;
        }
        let plain = transport.get(obj_id, &file_id)?;
        let rev: SyncRevision = serde_json::from_slice(&plain)
            .map_err(|_| "원격 동기화 버전 형식이 올바르지 않습니다".to_string())?;
        if rev.version != 1 || rev.object_id != obj_id {
            return Err("원격 동기화 버전 검증에 실패했습니다".into());
        }
        if let Some(ref entry) = rev.entry {
            validate_remote_entry(entry)?;
        }
        downloaded.push((file_id, rev));
    }

    for (file_id, rev) in &downloaded {
        let rev_id = rev.object_id.to_string();
        store
            .revision_parents
            .insert(rev_id.clone(), rev.parent_ids.clone());
        store.seen_remote_objects.insert(rev_id, file_id.clone());
        summary.downloaded_revisions += 1;
        if let Some(j) = &rev.journal {
            merge_journal(store, j.clone());
        }
        for j in &rev.journals {
            merge_journal(store, j.clone());
        }
        for t in &rev.templates {
            merge_template(store, t.clone());
        }
        if let Some(ref nick) = rev.nickname {
            merge_nickname(store, nick.clone());
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
                !all_ids.iter().any(|other| {
                    other != *id && is_ancestor(&store.revision_parents, id, other)
                })
            })
            .cloned()
            .collect();

        let mut new_leaves: Vec<(String, Entry)> = revs
            .into_iter()
            .filter(|(id, _)| leaf_ids.contains(id))
            .collect();

        store.remote_heads.insert(entry_id.clone(), leaf_ids.clone());

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
                    store.sync_base_entries.insert(entry_id, merged);
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
                store.sync_base_entries.insert(entry_id.clone(), first.clone());
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
    if pull_only && (!store.sync_conflicts.is_empty() || store.pending_resolutions.is_empty()) { return Ok(summary); }

    // 2. Push pending conflict resolutions
    let resolutions = std::mem::take(&mut store.pending_resolutions);
    for resolution in resolutions {
        let entry_id = resolution.conflict.entry_id;
        let current = store
            .entries
            .iter()
            .find(|e| e.id == entry_id)
            .cloned()
            .or(resolution.chosen);
        if let Some(entry) = current {
            upload_entry_revision(store, data_dir, transport, &entry, &mut summary)?;
            save_checkpoint(store)?;
        }
    }

    if pull_only { return Ok(summary); }

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
        upload_entry_revision(store, data_dir, transport, &entry, &mut summary)?;
        save_checkpoint(store)?;
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
    let dirty_nickname = if store.sync_base_nickname.as_deref() != Some(store.nickname.as_str())
        && !store.nickname.is_empty()
    {
        Some(store.nickname.clone())
    } else {
        None
    };

    if !dirty_journals.is_empty() || !dirty_templates.is_empty() || dirty_nickname.is_some() {
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
            nickname: dirty_nickname.clone(),
            media_manifests: HashMap::new(),
        };
        let bytes = serde_json::to_vec(&rev).map_err(err)?;
        let file_id = transport.put(object_id, &bytes)?;
        let rev_id = object_id.to_string();
        store.seen_remote_objects.insert(rev_id.clone(), file_id);
        store.revision_parents.insert(rev_id, Vec::new());
        for j in dirty_journals {
            store.sync_base_journals.insert(j.id.clone(), j);
        }
        for t in dirty_templates {
            store.sync_base_templates.insert(t.id.clone(), t);
        }
        if let Some(nick) = dirty_nickname {
            store.sync_base_nickname = Some(nick);
        }
        summary.uploaded_revisions += 1;
        save_checkpoint(store)?;
    }

    store.last_sync_at = Some(now());
    store.last_sync_error = None;
    save_checkpoint(store)?;
    Ok(summary)
}

fn upload_entry_revision(
    store: &mut Store,
    data_dir: &Path,
    transport: &impl RemoteTransport,
    entry: &Entry,
    summary: &mut SyncSummary,
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
        nickname: None,
        media_manifests: entry_manifests,
    };
    let plain = serde_json::to_vec(&rev).map_err(err)?;
    let file_id = transport.put(object_id, &plain)?;
    let rev_id = object_id.to_string();
    store.seen_remote_objects.insert(rev_id.clone(), file_id);
    store.revision_parents.insert(rev_id.clone(), parent_ids);
    store.remote_heads.insert(entry.id.clone(), vec![rev_id]);
    store
        .sync_base_entries
        .insert(entry.id.clone(), entry.clone());
    if let Some(j) = journal {
        store.sync_base_journals.insert(j.id.clone(), j);
    }
    summary.uploaded_revisions += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Attachment,
        drive_transport::{ChangeBatch, DriveChange, DriveFile},
        resolve_conflict,
        sync_crypto,
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
            cloud.files.push((file_id.clone(), name.clone(), ciphertext));
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
            let file_id=self.0.put(id, bytes)?;
            let mut cloud=self.0.cloud.borrow_mut();
            let name=crate::drive_transport::v2_name(id);
            cloud.files.last_mut().unwrap().1=name.clone();
            cloud.changes.last_mut().unwrap().file.as_mut().unwrap().name=name;
            Ok(file_id)
        }
        fn get(&self, id: Uuid, file_id: &str) -> Result<Vec<u8>, String> { self.0.get(id,file_id) }
    }
    impl RemoteTransport for ScopedMock<'_> {
        fn list(&self) -> Result<Vec<DriveFile>, String> {
            Ok(self.0.list()?.into_iter().filter_map(|mut f| {f.name=crate::drive_transport::decode_v2_name(&f.name)?;Some(f)}).collect())
        }
        fn start_cursor(&self) -> Result<String, String> { self.0.start_cursor() }
        fn changes(&self, cursor: &str) -> Result<ChangeBatch, String> {
            let mut batch=self.0.changes(cursor)?;
            for change in &mut batch.changes { if let Some(f)=&mut change.file {f.name=crate::drive_transport::decode_v2_name(&f.name).unwrap_or_default();} }
            Ok(batch)
        }
    }

    #[test]
    fn migrated_entries_and_media_restore_on_another_device_without_touching_legacy() {
        let cloud=RefCell::new(MockCloud::default());
        let legacy=MockClient {cloud:&cloud,key:crate::drive_transport::derive_account_sync_key("test@example.com")};
        let shared_key=sync_crypto::generate_key().unwrap();
        let v2=ScopedMock(MockClient {cloud:&cloud,key:shared_key});
        let dir_a=temp_device_dir("migration-a");let dir_b=temp_device_dir("migration-b");
        let journal=Journal::default();let attachment_id=Uuid::new_v4().to_string();
        let bytes=b"\x89PNG\r\n\x1a\nimage";
        fs::write(dir_a.join("media").join(&attachment_id),bytes).unwrap();
        let entry=Entry {journal_id:journal.id.clone(),body:"**기존 기록**".into(),attachments:vec![Attachment {id:attachment_id.clone(),name:"photo.png".into(),mime:"image/png".into(),bytes:bytes.len()}],..Entry::default()};
        let mut a=Store {journals:vec![journal],entries:vec![entry.clone()],..Store::default()};
        sync_with_transport(&mut a,&dir_a,&legacy,|_|Ok(())).unwrap();
        let legacy_count=cloud.borrow().files.len();
        a.entries[0].body="**이전 후 새 편집**".into();
        crate::prepare_v2_migration(&mut a,&dir_a,&legacy,"test@example.com",||Ok(()),|_|Ok(())).unwrap();
        assert_eq!(cloud.borrow().files.len(),legacy_count);
        assert_eq!(a.sync_format,2);
        assert_eq!(a.sync_account.as_deref(),Some("test@example.com"));
        sync_with_transport(&mut a,&dir_a,&v2,|_|Ok(())).unwrap();
        assert!(cloud.borrow().files.len()>legacy_count);
        assert!(cloud.borrow().files.iter().skip(legacy_count).all(|(_,name,_)|crate::sync_engine::parse_revision_uuid(name).is_none()));
        let mut b=Store::default();
        sync_with_transport(&mut b,&dir_b,&v2,|_|Ok(())).unwrap();
        assert_eq!(b.entries[0].body,"**이전 후 새 편집**");
        assert_eq!(fs::read(dir_b.join("media").join(attachment_id)).unwrap(),bytes);
        assert_eq!(cloud.borrow().files.iter().take(legacy_count).count(),legacy_count);
        fs::remove_dir_all(dir_a).unwrap();fs::remove_dir_all(dir_b).unwrap();
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
        let cloud=RefCell::new(MockCloud::default());
        let legacy=MockClient {cloud:&cloud,key:[3u8;32]};
        let dir=temp_device_dir("migration-conflict");
        let mut store=Store {sync_conflicts:vec![SyncConflict {id:"conflict".into(),entry_id:"entry".into(),local:None,remote:None}],..Store::default()};
        let called=std::cell::Cell::new(false);
        let result=crate::prepare_v2_migration(&mut store,&dir,&legacy,"account",||{called.set(true);Ok(())},|_|Ok(()));
        assert!(result.is_err());assert!(!called.get());assert_eq!(store.sync_format,0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn concurrent_local_edit_is_preserved_and_conflicts_on_retry() {
        let cloud = RefCell::new(MockCloud::default());
        let client = MockClient { cloud: &cloud, key: sync_crypto::generate_key().unwrap() };
        let dir_a = temp_device_dir("race-a");
        let dir_b = temp_device_dir("race-b");
        let journal = Journal::default();
        let entry = Entry { journal_id: journal.id.clone(), body: "original".into(), ..Entry::default() };
        let mut a = Store { journals: vec![journal], entries: vec![entry], ..Store::default() };
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
                fs::write(dir_b.join("journal.json"), serde_json::to_vec(&*store).unwrap()).unwrap();
            }
            crate::commit_sync_checkpoint(&live, &dir_b, &mut base, checkpoint)
        });
        assert!(result.is_err());
        let persisted: Store = serde_json::from_slice(&fs::read(dir_b.join("journal.json")).unwrap()).unwrap();
        assert_eq!(persisted.entries[0].body, "local edit");
        let mut retry = live.into_inner().unwrap();
        sync_with_transport(&mut retry, &dir_b, &client, |_| Ok(())).unwrap();
        assert_eq!(retry.entries[0].body, "local edit");
        assert_eq!(retry.sync_conflicts.len(), 1);
        assert_eq!(retry.sync_conflicts[0].remote.as_ref().unwrap().body, "remote edit");
        fs::remove_dir_all(dir_a).unwrap();
        fs::remove_dir_all(dir_b).unwrap();
    }

    #[test]
    fn syncs_entries_media_journals_templates_and_restores_on_new_device() {
        let cloud = RefCell::new(MockCloud::default());
        let key = sync_crypto::generate_key().unwrap();
        let client = MockClient {
            cloud: &cloud,
            key,
        };

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
            body: "## 오늘\n**사진**과 함께 남긴 기록\n\n- [x] 산책\n- [ ] 독서".into(),
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
            store_b.entries.iter().find(|e| e.id == entry_a.id).unwrap().body,
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
            store_c.entries.iter().find(|e| e.id == entry_a.id).unwrap().body,
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
        let client = MockClient {
            cloud: &cloud,
            key,
        };

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
        let client = MockClient {
            cloud: &cloud,
            key,
        };

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
}
