//! Layered diary memory for Niki: cached summaries, local search and prompt assembly.
//! Everything here is pure and local; only the assembled prompt ever leaves the device.
use crate::Entry;
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EntrySummary {
    pub updated_at: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MonthSummary {
    pub signature: String,
    pub text: String,
}

/// Entries at most this long are their own summary and are never sent to be summarized.
pub const SUMMARY_MIN_CHARS: usize = 300;
const BATCH_ENTRIES: usize = 12;
const BATCH_BYTES: usize = 30_000;
const SUMMARY_INPUT_CHARS: usize = 3_000;
const RECENT_BUDGET: usize = 30_000;
const RECENT_MAX: usize = 14;
const ENTRY_CAP_CHARS: usize = 3_000;
const RELEVANT_BUDGET: usize = 24_000;
const RELEVANT_MAX: usize = 6;
const MONTH_BUDGET: usize = 16_000;
const DIGEST_BUDGET: usize = 24_000;
const FALLBACK_EXCERPT_CHARS: usize = 160;

pub fn truncate_chars(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

fn active(entries: &[Entry]) -> Vec<&Entry> {
    entries
        .iter()
        .filter(|e| {
            e.deleted_at.is_none() && !(e.title.trim().is_empty() && e.body.trim().is_empty())
        })
        .collect()
}

fn needs_summary(e: &Entry) -> bool {
    e.body.trim().chars().count() > SUMMARY_MIN_CHARS
}

fn month_of(e: &Entry) -> &str {
    e.date.get(..7).unwrap_or("")
}

fn entry_date(e: &Entry) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(e.date.get(..10)?, "%Y-%m-%d").ok()
}

/// One line of gist for an entry: its cached summary, or the entry itself when short.
pub fn digest(e: &Entry, cache: &HashMap<String, EntrySummary>) -> String {
    let body = e.body.trim();
    if !needs_summary(e) {
        return truncate_chars(body, SUMMARY_MIN_CHARS);
    }
    match cache.get(&e.id) {
        Some(s) if s.updated_at == e.updated_at => s.text.clone(),
        _ => truncate_chars(body, FALLBACK_EXCERPT_CHARS),
    }
}

// ---------------------------------------------------------------- summary jobs

pub fn stale_entries<'a>(
    entries: &'a [Entry],
    cache: &HashMap<String, EntrySummary>,
) -> Vec<&'a Entry> {
    let mut stale: Vec<&Entry> = active(entries)
        .into_iter()
        .filter(|e| {
            needs_summary(e) && cache.get(&e.id).map_or(true, |s| s.updated_at != e.updated_at)
        })
        .collect();
    stale.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.id.cmp(&b.id)));
    stale
}

fn signature(list: &[&Entry]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for e in list {
        e.id.hash(&mut hasher);
        e.updated_at.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

/// Months that are over (older than the newest month with entries), have at least two entries,
/// and whose overview is missing or out of date.
fn pending_months<'a>(
    entries: &'a [Entry],
    months: &HashMap<String, MonthSummary>,
) -> Vec<(String, Vec<&'a Entry>, String)> {
    let mut grouped: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
    for e in active(entries) {
        let month = month_of(e);
        if !month.is_empty() {
            grouped.entry(month).or_default().push(e);
        }
    }
    let latest = grouped.keys().next_back().copied();
    let mut out = Vec::new();
    for (month, mut list) in grouped {
        if Some(month) == latest || list.len() < 2 {
            continue;
        }
        list.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.id.cmp(&b.id)));
        let sig = signature(&list);
        if months.get(month).map_or(true, |m| m.signature != sig) {
            out.push((month.to_owned(), list, sig));
        }
    }
    out
}

pub fn pending_count(
    entries: &[Entry],
    entry_cache: &HashMap<String, EntrySummary>,
    month_cache: &HashMap<String, MonthSummary>,
) -> usize {
    stale_entries(entries, entry_cache).len() + pending_months(entries, month_cache).len()
}

/// Drops cached summaries whose diary or month no longer exists.
pub fn prune(
    entries: &[Entry],
    entry_cache: &mut HashMap<String, EntrySummary>,
    month_cache: &mut HashMap<String, MonthSummary>,
) {
    let live = active(entries);
    let ids: HashSet<&str> = live.iter().map(|e| e.id.as_str()).collect();
    entry_cache.retain(|id, _| ids.contains(id.as_str()));
    let mut count: HashMap<&str, usize> = HashMap::new();
    for e in &live {
        *count.entry(month_of(e)).or_insert(0) += 1;
    }
    month_cache.retain(|month, _| count.get(month.as_str()).copied().unwrap_or(0) >= 2);
}

pub enum Job {
    Entries {
        prompt: String,
        sent: Vec<(String, String)>,
    },
    Month {
        month: String,
        signature: String,
        prompt: String,
    },
}

/// The next piece of work: a batch of stale entries first, then one finished month.
pub fn next_job(
    entries: &[Entry],
    entry_cache: &HashMap<String, EntrySummary>,
    month_cache: &HashMap<String, MonthSummary>,
) -> Option<Job> {
    let stale = stale_entries(entries, entry_cache);
    if !stale.is_empty() {
        let mut bytes = 0usize;
        let mut batch: Vec<&Entry> = Vec::new();
        for e in stale {
            let cost = e.title.len() + e.body.len().min(SUMMARY_INPUT_CHARS * 3);
            if !batch.is_empty() && (batch.len() >= BATCH_ENTRIES || bytes + cost > BATCH_BYTES) {
                break;
            }
            bytes += cost;
            batch.push(e);
        }
        let items: Vec<Value> = batch
            .iter()
            .map(|e| {
                json!({
                    "id": e.id, "date": e.date, "title": e.title,
                    "body": truncate_chars(e.body.trim(), SUMMARY_INPUT_CHARS)
                })
            })
            .collect();
        let sent = batch
            .iter()
            .map(|e| (e.id.clone(), e.updated_at.clone()))
            .collect();
        return Some(Job::Entries {
            prompt: json!({ "entries": items }).to_string(),
            sent,
        });
    }
    let (month, list, sig) = pending_months(entries, month_cache).into_iter().next()?;
    let items: Vec<Value> = list
        .iter()
        .map(|e| json!({ "date": e.date, "title": e.title, "gist": digest(e, entry_cache) }))
        .collect();
    Some(Job::Month {
        prompt: json!({ "month": month, "entries": items }).to_string(),
        month,
        signature: sig,
    })
}

/// Reads the model's JSON array, keeping only summaries for ids that were actually sent.
pub fn parse_entry_summaries(output: &str, allowed: &HashSet<String>) -> Vec<(String, String)> {
    let (Some(start), Some(end)) = (output.find('['), output.rfind(']')) else {
        return Vec::new();
    };
    if end < start {
        return Vec::new();
    }
    let Ok(items) = serde_json::from_str::<Vec<Value>>(&output[start..=end]) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item["id"].as_str()?;
            let text = item["summary"].as_str()?.trim();
            if allowed.contains(id) && !text.is_empty() {
                Some((id.to_owned(), truncate_chars(text, 400)))
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------- search

fn bigrams(text: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let lowered = text.to_lowercase();
    for word in lowered.split(|c: char| !c.is_alphanumeric()) {
        let chars: Vec<char> = word.chars().collect();
        for pair in chars.windows(2) {
            set.insert(pair.iter().collect::<String>());
        }
    }
    set
}

fn month_range(year: i32, month: u32) -> Option<(NaiveDate, NaiveDate)> {
    let from = NaiveDate::from_ymd_opt(year, month, 1)?;
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)?
    };
    Some((from, next - Duration::days(1)))
}

fn year_range(year: i32) -> Option<(NaiveDate, NaiveDate)> {
    Some((
        NaiveDate::from_ymd_opt(year, 1, 1)?,
        NaiveDate::from_ymd_opt(year, 12, 31)?,
    ))
}

/// First number written directly before `marker` (e.g. 3 in "3월"), with its digit count.
fn number_before(chars: &[char], marker: char) -> Option<(u32, usize)> {
    for (i, c) in chars.iter().enumerate() {
        if *c != marker || i == 0 || !chars[i - 1].is_ascii_digit() {
            continue;
        }
        let mut start = i;
        while start > 0 && chars[start - 1].is_ascii_digit() {
            start -= 1;
        }
        let digits: String = chars[start..i].iter().collect();
        return digits.parse::<u32>().ok().map(|n| (n, i - start));
    }
    None
}

/// Understands "2025년", "3월", "작년", "지난달", "지난주", "어제" and similar expressions.
fn date_range(query: &str, today: NaiveDate) -> Option<(NaiveDate, NaiveDate)> {
    let chars: Vec<char> = query.chars().collect();
    let year = number_before(&chars, '년')
        .filter(|&(n, len)| len == 4 && (1900..=2200).contains(&n))
        .map(|(n, _)| n as i32);
    let month = number_before(&chars, '월')
        .filter(|&(n, len)| len <= 2 && (1..=12).contains(&n))
        .map(|(n, _)| n);
    if let Some(m) = month {
        let y = year.unwrap_or(if m > today.month() {
            today.year() - 1
        } else {
            today.year()
        });
        return month_range(y, m);
    }
    if let Some(y) = year {
        return year_range(y);
    }
    let has = |word: &str| query.contains(word);
    let (y, m) = (today.year(), today.month());
    let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    if has("재작년") {
        return year_range(y - 2);
    }
    if has("작년") || has("지난해") {
        return year_range(y - 1);
    }
    if has("올해") {
        return year_range(y);
    }
    if has("지난달") || has("저번달") || has("저번 달") {
        return if m == 1 {
            month_range(y - 1, 12)
        } else {
            month_range(y, m - 1)
        };
    }
    if has("이번달") || has("이번 달") {
        return month_range(y, m);
    }
    if has("지난주") || has("저번주") || has("저번 주") {
        let start = monday - Duration::days(7);
        return Some((start, start + Duration::days(6)));
    }
    if has("이번주") || has("이번 주") {
        return Some((monday, today));
    }
    if has("그저께") || has("그제") {
        let day = today - Duration::days(2);
        return Some((day, day));
    }
    if has("어제") {
        let day = today - Duration::days(1);
        return Some((day, day));
    }
    if has("오늘") {
        return Some((today, today));
    }
    None
}

/// Scores entries by IDF-weighted character-bigram overlap (works for Korean without a
/// morphological analyzer); title hits count triple.
fn rank<'a>(pool: &[&'a Entry], terms: &[String]) -> Vec<&'a Entry> {
    if terms.is_empty() || pool.is_empty() {
        return Vec::new();
    }
    let docs: Vec<String> = pool
        .iter()
        .map(|e| {
            format!("{} {} {} {}", e.title, e.body, e.tags.join(" "), e.location).to_lowercase()
        })
        .collect();
    let titles: Vec<String> = pool.iter().map(|e| e.title.to_lowercase()).collect();
    let hits: Vec<Vec<bool>> = docs
        .iter()
        .map(|doc| terms.iter().map(|t| doc.contains(t.as_str())).collect())
        .collect();
    let n = pool.len() as f64;
    let df: Vec<f64> = (0..terms.len())
        .map(|i| hits.iter().filter(|row| row[i]).count() as f64)
        .collect();
    let needed = if terms.len() >= 4 { 2 } else { 1 };
    let mut scored: Vec<(f64, &Entry)> = Vec::new();
    for (idx, e) in pool.iter().enumerate() {
        let (mut score, mut matched) = (0.0f64, 0usize);
        for (i, term) in terms.iter().enumerate() {
            if !hits[idx][i] || df[i] == 0.0 || (pool.len() >= 10 && df[i] / n > 0.4) {
                continue;
            }
            matched += 1;
            let weight = (n / df[i]).ln() + 1.0;
            score += weight * if titles[idx].contains(term.as_str()) { 3.0 } else { 1.0 };
        }
        if matched >= needed {
            scored.push((score, *e));
        }
    }
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.date.cmp(&a.1.date))
    });
    scored.into_iter().map(|(_, e)| e).collect()
}

/// Entries relevant to `query`: restricted to a recognized period when there is one,
/// otherwise (or when nothing in the period matches) the best keyword matches overall.
pub fn search<'a>(
    entries: &'a [Entry],
    query: &str,
    today: NaiveDate,
    limit: usize,
) -> Vec<&'a Entry> {
    let all = active(entries);
    let terms: Vec<String> = bigrams(query).into_iter().collect();
    if let Some((from, to)) = date_range(query, today) {
        let pool: Vec<&Entry> = all
            .iter()
            .copied()
            .filter(|e| entry_date(e).map_or(false, |d| d >= from && d <= to))
            .collect();
        let mut found = rank(&pool, &terms);
        if found.is_empty() {
            found = pool;
            found.sort_by(|a, b| b.date.cmp(&a.date));
        }
        if !found.is_empty() {
            found.truncate(limit);
            return found;
        }
    }
    let mut found = rank(&all, &terms);
    found.truncate(limit);
    found
}

// --------------------------------------------------------------- prompt context

fn full_json(e: &Entry) -> Value {
    json!({
        "date": e.date, "title": e.title, "body": truncate_chars(e.body.trim(), ENTRY_CAP_CHARS),
        "tags": e.tags, "location": e.location, "weather": e.weather
    })
}

fn full_cost(e: &Entry) -> usize {
    e.title.len() + e.body.trim().len().min(ENTRY_CAP_CHARS * 3)
}

/// Layered context: monthly overviews and one-line notes for everything older, full text for
/// entries matching the message, and full text for the most recent entries.
pub fn diary_context(
    entries: &[Entry],
    entry_cache: &HashMap<String, EntrySummary>,
    month_cache: &HashMap<String, MonthSummary>,
    query: &str,
    today: NaiveDate,
) -> Value {
    let mut all = active(entries);
    all.sort_by(|a, b| {
        b.date
            .cmp(&a.date)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });

    let mut used: HashSet<&str> = HashSet::new();
    let mut recent: Vec<&Entry> = Vec::new();
    let mut bytes = 0usize;
    for e in all.iter().copied() {
        let cost = full_cost(e);
        if recent.len() >= RECENT_MAX || (!recent.is_empty() && bytes + cost > RECENT_BUDGET) {
            break;
        }
        bytes += cost;
        used.insert(e.id.as_str());
        recent.push(e);
    }
    recent.reverse();

    let mut relevant: Vec<&Entry> = Vec::new();
    bytes = 0;
    for e in search(entries, query, today, RELEVANT_MAX + RECENT_MAX) {
        let cost = full_cost(e);
        if used.contains(e.id.as_str()) {
            continue;
        }
        if relevant.len() >= RELEVANT_MAX || (!relevant.is_empty() && bytes + cost > RELEVANT_BUDGET)
        {
            break;
        }
        bytes += cost;
        used.insert(e.id.as_str());
        relevant.push(e);
    }
    relevant.sort_by(|a, b| a.date.cmp(&b.date));

    let mut months: Vec<(&String, &MonthSummary)> = month_cache.iter().collect();
    months.sort_by(|a, b| b.0.cmp(a.0));
    let mut overview: Vec<Value> = Vec::new();
    bytes = 0;
    for (month, summary) in months {
        let cost = summary.text.len() + 16;
        if bytes + cost > MONTH_BUDGET {
            break;
        }
        bytes += cost;
        overview.push(json!({ "month": month, "overview": summary.text }));
    }
    overview.reverse();

    let mut notes: Vec<Value> = Vec::new();
    let (mut omitted, mut full) = (0usize, false);
    bytes = 0;
    for e in all.iter().copied() {
        if used.contains(e.id.as_str()) {
            continue;
        }
        let gist = digest(e, entry_cache);
        let cost = e.title.len() + gist.len() + 24;
        if full || bytes + cost > DIGEST_BUDGET {
            full = true;
            omitted += 1;
            continue;
        }
        bytes += cost;
        notes.push(json!({ "date": e.date, "title": e.title, "gist": gist }));
    }
    notes.reverse();

    json!({
        "counts": {
            "total_entries": all.len(),
            "most_recent_entries_in_full": recent.len(),
            "entries_matching_the_message_in_full": relevant.len(),
            "older_entries_with_one_line_notes": notes.len(),
            "oldest_entries_not_listed": omitted
        },
        "monthly_overviews_oldest_to_newest": overview,
        "older_entry_notes_oldest_to_newest": notes,
        "entries_matching_the_message": relevant.iter().map(|e| full_json(e)).collect::<Vec<_>>(),
        "most_recent_entries_oldest_to_newest": recent.iter().map(|e| full_json(e)).collect::<Vec<_>>()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u32, date: &str, title: &str, body: &str) -> Entry {
        serde_json::from_value(json!({
            "id": format!("e{id}"), "journal_id": "j", "title": title, "body": body, "date": date,
            "created_at": "v", "updated_at": "v1", "favorite": false, "tags": [], "location": "",
            "weather": "", "attachments": [], "deleted_at": null, "external_id": null
        }))
        .unwrap()
    }

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn date_expressions_become_ranges() {
        let today = day(2026, 11, 11); // Wednesday
        assert_eq!(date_range("작년 여름에 뭐 했지", today), year_range(2025));
        assert_eq!(date_range("지난달 일기", today), month_range(2026, 10));
        assert_eq!(date_range("3월에 있었던 일", today), month_range(2026, 3));
        assert_eq!(date_range("12월에 있었던 일", today), month_range(2025, 12));
        assert_eq!(date_range("2024년 5월", today), month_range(2024, 5));
        assert_eq!(date_range("어제 어땠어", today), Some((day(2026, 11, 10), day(2026, 11, 10))));
        assert_eq!(date_range("지난주", today), Some((day(2026, 11, 2), day(2026, 11, 8))));
        assert_eq!(date_range("월요일에", today), None);
        assert_eq!(date_range("그냥 이야기", today), None);
    }

    #[test]
    fn search_finds_old_entries_by_keyword_and_period() {
        let mut list = vec![
            entry(1, "2024-07-10", "제주도 여행", "친구와 제주도 바닷가를 걸었다."),
            entry(2, "2024-08-01", "회사", "회의가 길었고 피곤했다."),
            entry(3, "2026-11-01", "산책", "공원에서 산책했다."),
        ];
        for i in 10..30 {
            list.push(entry(i, "2025-03-01", "평범한 하루", "그냥 평범한 하루였다."));
        }
        let today = day(2026, 11, 11);
        let hits = search(&list, "제주도 여행 이야기 해줘", today, 3);
        assert_eq!(hits[0].id, "e1");
        let in_period = search(&list, "2024년 8월에 어땠지", today, 3);
        assert_eq!(in_period[0].id, "e2");
        assert!(search(&list, "전혀관계없는질의", today, 3).is_empty());
    }

    #[test]
    fn summary_jobs_follow_staleness() {
        let long = "가".repeat(400);
        let list = vec![
            entry(1, "2026-09-01", "하나", &long),
            entry(2, "2026-09-02", "둘", "짧은 일기"),
            entry(3, "2026-09-03", "셋", &long),
            entry(4, "2026-10-01", "넷", "짧은 일기"),
        ];
        let (mut ec, mut mc) = (HashMap::new(), HashMap::new());
        assert_eq!(stale_entries(&list, &ec).len(), 2);
        assert_eq!(pending_count(&list, &ec, &mc), 3); // 2 entries + September
        let Some(Job::Entries { sent, .. }) = next_job(&list, &ec, &mc) else {
            panic!("entries first");
        };
        assert_eq!(sent.len(), 2);
        for (id, updated_at) in sent {
            ec.insert(id, EntrySummary { updated_at, text: "요약".into() });
        }
        let Some(Job::Month { month, signature, .. }) = next_job(&list, &ec, &mc) else {
            panic!("month next");
        };
        assert_eq!(month, "2026-09");
        mc.insert(month, MonthSummary { signature, text: "9월 개요".into() });
        assert_eq!(pending_count(&list, &ec, &mc), 0);
        assert!(next_job(&list, &ec, &mc).is_none());
        let mut changed = list.clone();
        changed[0].updated_at = "v2".into();
        assert_eq!(pending_count(&changed, &ec, &mc), 2); // the entry and its month
        changed.truncate(1);
        prune(&changed, &mut ec, &mut mc);
        assert_eq!(ec.len(), 1);
        assert!(mc.is_empty());
    }

    #[test]
    fn model_output_is_filtered_to_sent_ids() {
        let allowed: HashSet<String> = ["e1".to_owned(), "e2".to_owned()].into();
        let out = "```json\n[{\"id\":\"e1\",\"summary\":\" 산책 \"},{\"id\":\"zz\",\"summary\":\"x\"},{\"id\":\"e2\",\"summary\":\"\"}]\n```";
        assert_eq!(parse_entry_summaries(out, &allowed), vec![("e1".to_owned(), "산책".to_owned())]);
        assert!(parse_entry_summaries("요약 불가", &allowed).is_empty());
    }

    #[test]
    fn context_is_layered_and_bounded() {
        let mut list = Vec::new();
        for i in 0..400u32 {
            let date = format!("{}-{:02}-{:02}", 2024 + i / 336, (i / 28) % 12 + 1, i % 28 + 1);
            list.push(entry(i, &date, &format!("제목{i}"), &"가".repeat(2_000)));
        }
        let (ec, mut mc) = (HashMap::new(), HashMap::new());
        mc.insert("2024-01".to_owned(), MonthSummary { signature: "s".into(), text: "첫 달 개요".into() });
        let ctx = diary_context(&list, &ec, &mc, "제목7 이야기", day(2026, 11, 11));
        assert_eq!(ctx["counts"]["total_entries"], 400);
        assert!(ctx["counts"]["most_recent_entries_in_full"].as_u64().unwrap() <= RECENT_MAX as u64);
        assert!(ctx["counts"]["oldest_entries_not_listed"].as_u64().unwrap() > 0);
        assert_eq!(ctx["monthly_overviews_oldest_to_newest"][0]["overview"], "첫 달 개요");
        assert!(ctx.to_string().len() < 130_000, "{}", ctx.to_string().len());
    }
}
