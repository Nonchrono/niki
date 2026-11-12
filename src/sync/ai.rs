//! Mergeable AI data carried by the existing encrypted, checkpointed Drive revisions.
//! Messages are immutable records; their tombstones always win. Derived data uses
//! Lamport versions, so clocks/time zones on individual computers do not affect merging.
use crate::{
    Store,
    codex::ChatMessage,
    memory::{ConversationSummary, MonthSummary},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Stamp {
    pub clock: u64,
    pub device: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub stamp: Stamp,
    pub value: Option<Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub clock: u64,
    pub records: BTreeMap<String, Record>,
    pub uploaded: BTreeMap<String, Stamp>,
    /// Older readers marked AI revisions as seen without applying their contents.
    pub remote_initialized: bool,
}

pub type Changes = BTreeMap<String, Record>;

pub fn capture(store: &mut Store) {
    if store.device_id.is_empty() {
        store.device_id = uuid::Uuid::new_v4().to_string();
    }
    let entries: HashMap<_, _> = store.entries.iter().map(|entry| (&entry.id, entry)).collect();
    store.codex_search.retain(|id, search| {
        entries.get(id).is_some_and(|entry| {
            entry.deleted_at.is_none()
                && !store.codex_excluded_journals.contains(&entry.journal_id)
                && search.matches(entry)
        })
    });
    // Preserve the legacy conversation's order on its first upload.
    let mut desired = Vec::new();
    let mut question = None;
    for message in &mut store.codex_conversation {
        if matches!(message.role, crate::codex::Role::User) {
            question = Some(message.id.clone());
        } else if message.reply_to.is_none() {
            message.reply_to = question.clone();
        }
        desired.push((
            format!("chat/{}", message.id),
            serde_json::to_value(message).unwrap(),
        ));
    }
    if let Some(summary) = &mut store.codex_conversation_summary {
        if summary.signature.is_empty() {
            summary.signature =
                crate::memory::history_signature(&store.codex_conversation, &summary.through_id);
        }
        desired.push((
            "conversation".into(),
            serde_json::to_value(summary).unwrap(),
        ));
    }
    let mut derived = BTreeMap::new();
    for (month, summary) in &store.codex_month_summaries {
        derived.insert(
            format!("month/{month}"),
            serde_json::to_value(summary).unwrap(),
        );
    }
    for (id, search) in &store.codex_search {
        derived.insert(
            format!("search/{id}"),
            serde_json::to_value(search).unwrap(),
        );
    }
    for id in &store.codex_excluded_journals {
        derived.insert(format!("excluded/{id}"), Value::Bool(true));
    }
    desired.extend(derived);
    let keys: HashSet<_> = desired.iter().map(|(key, _)| key.clone()).collect();
    let mut updates: Vec<_> = desired
        .into_iter()
        .map(|(key, value)| (key, Some(value)))
        .collect();
    updates.extend(
        store
            .ai_sync
            .records
            .keys()
            .filter(|key| !keys.contains(*key))
            .cloned()
            .map(|key| (key, None)),
    );
    for (key, value) in updates {
        if store
            .ai_sync
            .records
            .get(&key)
            .is_some_and(|record| record.value == value)
        {
            continue;
        }
        store.ai_sync.clock += 1;
        store.ai_sync.records.insert(
            key,
            Record {
                stamp: Stamp {
                    clock: store.ai_sync.clock,
                    device: store.device_id.clone(),
                },
                value,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{MessageKind, Role};

    fn ask(store: &mut Store, text: &str) {
        store
            .codex_conversation
            .push(ChatMessage::new(Role::User, text.into(), MessageKind::Chat));
        store.codex_conversation.push(ChatMessage::new(
            Role::Assistant,
            format!("answer {text}"),
            MessageKind::Chat,
        ));
        capture(store);
    }
    fn exchange(a: &mut Store, b: &mut Store) {
        let aa = a.ai_sync.records.clone();
        let bb = b.ai_sync.records.clone();
        merge(a, &bb);
        project(a);
        merge(b, &aa);
        project(b);
    }
    fn history(store: &Store) -> Value {
        serde_json::to_value(&store.codex_conversation).unwrap()
    }

    #[test]
    fn simultaneous_turns_converge_without_interleaving_questions_and_answers() {
        let mut a = Store::default();
        ask(&mut a, "shared");
        let mut b = a.clone();
        b.device_id = "another-device".into();
        ask(&mut a, "from A");
        ask(&mut b, "from B");
        exchange(&mut a, &mut b);
        assert_eq!(history(&a), history(&b));
        assert_eq!(a.codex_conversation.len(), 6);
        for pair in a.codex_conversation.chunks_exact(2) {
            assert_eq!(pair[1].reply_to.as_deref(), Some(pair[0].id.as_str()));
        }
        let once = history(&a);
        exchange(&mut a, &mut b);
        assert_eq!(once, history(&a));
    }

    #[test]
    fn clear_keeps_offline_new_turns_but_never_resurrects_deleted_messages() {
        let mut a = Store::default();
        ask(&mut a, "old");
        let mut offline = a.clone();
        offline.device_id = "offline".into();
        a.codex_conversation.clear();
        capture(&mut a);
        ask(&mut offline, "new offline turn");
        exchange(&mut a, &mut offline);
        assert_eq!(a.codex_conversation.len(), 2);
        assert_eq!(a.codex_conversation[0].content, "new offline turn");
        assert_eq!(history(&a), history(&offline));
        let deleted_id = a
            .ai_sync
            .records
            .iter()
            .find(|(_, r)| r.value.is_none())
            .unwrap()
            .0
            .clone();
        let mut stale = offline.ai_sync.records[&deleted_id].clone();
        stale.stamp.clock += 1000;
        let mut old = ChatMessage::new(Role::User, "must stay deleted".into(), MessageKind::Chat);
        old.id = deleted_id.strip_prefix("chat/").unwrap().into();
        stale.value = Some(serde_json::to_value(old).unwrap());
        merge(&mut a, &Changes::from([(deleted_id, stale)]));
        project(&mut a);
        assert_eq!(a.codex_conversation.len(), 2);
    }

    #[test]
    fn late_reply_to_deleted_question_is_hidden() {
        let mut a = Store::default();
        ask(&mut a, "old");
        let mut b = a.clone();
        b.device_id = "other".into();
        a.codex_conversation.clear();
        capture(&mut a);
        b.codex_conversation.push(ChatMessage::new(
            Role::Assistant,
            "late".into(),
            MessageKind::Chat,
        ));
        capture(&mut b);
        exchange(&mut a, &mut b);
        assert!(a.codex_conversation.is_empty());
    }

    #[test]
    fn merged_chat_invalidates_summary_that_did_not_cover_the_new_prefix() {
        let mut a = Store::default();
        ask(&mut a, "first");
        let mut b = a.clone();
        b.device_id = "other".into();
        ask(&mut a, "A");
        ask(&mut b, "B");
        let through = a.codex_conversation.last().unwrap().id.clone();
        a.codex_conversation_summary = Some(ConversationSummary {
            through_id: through.clone(),
            text: "summary A".into(),
            signature: crate::memory::history_signature(&a.codex_conversation, &through),
        });
        capture(&mut a);
        exchange(&mut a, &mut b);
        if a.codex_conversation_summary.is_some() {
            let summary = a.codex_conversation_summary.as_ref().unwrap();
            assert_eq!(
                summary.signature,
                crate::memory::history_signature(&a.codex_conversation, &summary.through_id)
            );
        }
        assert_eq!(history(&a), history(&b));
    }

    #[test]
    fn malformed_keys_and_vectors_are_rejected_before_merging() {
        let record = Record {
            stamp: Stamp {
                clock: 1,
                device: "device".into(),
            },
            value: Some(Value::Bool(true)),
        };
        assert!(validate(&Changes::from([("credentials".into(), record.clone())])).is_err());
        assert!(validate(&Changes::from([("search/id".into(), record.clone())])).is_err());
        assert!(validate(&Changes::from([("excluded/journal".into(), record)])).is_ok());
    }
}

pub fn pending_count(store: &Store) -> usize {
    store
        .ai_sync
        .records
        .iter()
        .filter(|(key, record)| store.ai_sync.uploaded.get(*key) != Some(&record.stamp))
        .count()
}

pub fn next_batch(store: &Store) -> Changes {
    let mut batch = Changes::new();
    let mut size = 0;
    for (key, record) in &store.ai_sync.records {
        if store.ai_sync.uploaded.get(key) == Some(&record.stamp) {
            continue;
        }
        let bytes = serde_json::to_vec(record).unwrap().len();
        if !batch.is_empty() && (size + bytes > 512 * 1024 || batch.len() >= 512) {
            break;
        }
        batch.insert(key.clone(), record.clone());
        size += bytes;
    }
    batch
}

pub fn acknowledge(store: &mut Store, changes: &Changes) {
    for (key, record) in changes {
        store
            .ai_sync
            .uploaded
            .insert(key.clone(), record.stamp.clone());
    }
}

pub fn merge(store: &mut Store, changes: &Changes) {
    for (key, remote) in changes {
        store.ai_sync.clock = store.ai_sync.clock.max(remote.stamp.clock);
        let replace = store.ai_sync.records.get(key).is_none_or(|local| {
            if key.starts_with("chat/") && local.value.is_none() != remote.value.is_none() {
                remote.value.is_none()
            } else {
                remote.stamp > local.stamp
            }
        });
        if replace {
            store.ai_sync.records.insert(key.clone(), remote.clone());
        }
        // A local winner still needs publishing, even if the remote has a higher clock.
        store
            .ai_sync
            .uploaded
            .insert(key.clone(), remote.stamp.clone());
    }
}

pub fn project(store: &mut Store) {
    let mut messages = Vec::new();
    store.codex_month_summaries.clear();
    store.codex_conversation_summary = None;
    store.codex_search.clear();
    store.codex_excluded_journals.clear();
    for (key, record) in &store.ai_sync.records {
        let Some(value) = &record.value else {
            continue;
        };
        if key.starts_with("chat/") {
            if let Ok(message) = serde_json::from_value::<ChatMessage>(value.clone()) {
                // A late reply must not resurrect a question cleared on another device.
                if message.reply_to.as_ref().is_some_and(|id| {
                    store
                        .ai_sync
                        .records
                        .get(&format!("chat/{id}"))
                        .is_none_or(|r| r.value.is_none())
                }) {
                    continue;
                }
                let parent = message
                    .reply_to
                    .as_ref()
                    .and_then(|id| store.ai_sync.records.get(&format!("chat/{id}")));
                let order = parent.map_or(&record.stamp, |r| &r.stamp).clone();
                messages.push((
                    order,
                    message.reply_to.is_some(),
                    record.stamp.clone(),
                    message,
                ));
            }
        } else if key == "conversation" {
            store.codex_conversation_summary = serde_json::from_value(value.clone()).ok();
        } else if let Some(month) = key.strip_prefix("month/") {
            if let Ok(summary) = serde_json::from_value(value.clone()) {
                store.codex_month_summaries.insert(month.into(), summary);
            }
        } else if let Some(id) = key.strip_prefix("search/") {
            if let Ok(search) = serde_json::from_value(value.clone()) {
                store.codex_search.insert(id.into(), search);
            }
        } else if let Some(id) = key.strip_prefix("excluded/") {
            if value == &Value::Bool(true) {
                store.codex_excluded_journals.insert(id.into());
            }
        }
    }
    messages.sort_by(|a, b| (&a.0, a.1, &a.2, &a.3.id).cmp(&(&b.0, b.1, &b.2, &b.3.id)));
    store.codex_conversation = messages
        .into_iter()
        .map(|(_, _, _, message)| message)
        .collect();
    if store.codex_conversation_summary.as_ref().is_some_and(|s| {
        s.signature != crate::memory::history_signature(&store.codex_conversation, &s.through_id)
    }) {
        store.codex_conversation_summary = None;
    }
}

pub fn validate(changes: &Changes) -> Result<(), String> {
    if changes.len() > 512 {
        return Err("AI 동기화 항목이 너무 많습니다".into());
    }
    for (key, record) in changes {
        let valid_key = key == "conversation"
            || ["chat/", "month/", "search/", "excluded/"]
                .iter()
                .any(|prefix| {
                    key.strip_prefix(prefix)
                        .is_some_and(|id| !id.is_empty() && id.len() <= 100)
                });
        if !valid_key
            || record.stamp.clock == 0
            || record.stamp.clock > u64::MAX / 2
            || record.stamp.device.is_empty()
            || record.stamp.device.len() > 100
        {
            return Err("AI 동기화 식별자가 올바르지 않습니다".into());
        }
        let Some(value) = &record.value else {
            continue;
        };
        let valid = if let Some(id) = key.strip_prefix("chat/") {
            serde_json::from_value::<ChatMessage>(value.clone()).is_ok_and(|m| {
                m.id == id
                    && m.content.len() <= 2_000_000
                    && m.sources.len() <= 100
                    && m.reply_to
                        .as_ref()
                        .is_none_or(|p| p != id && !p.is_empty() && p.len() <= 100)
            })
        } else if key == "conversation" {
            serde_json::from_value::<ConversationSummary>(value.clone()).is_ok_and(|s| {
                s.text.len() <= 100_000 && s.through_id.len() <= 100 && s.signature.len() <= 100
            })
        } else if key.starts_with("month/") {
            serde_json::from_value::<MonthSummary>(value.clone())
                .is_ok_and(|s| s.text.len() <= 100_000 && s.signature.len() <= 100)
        } else if key.starts_with("search/") {
            serde_json::from_value::<crate::embed::SharedIndex>(value.clone())
                .is_ok_and(|s| s.valid())
        } else {
            value == &Value::Bool(true)
        };
        if !valid {
            return Err("AI 동기화 데이터가 올바르지 않습니다".into());
        }
    }
    Ok(())
}
