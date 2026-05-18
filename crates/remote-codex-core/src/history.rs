use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use toml_edit::DocumentMut;
use walkdir::WalkDir;

use crate::{CodexPaths, RemoteCodexError, Result};

const DEFAULT_PROVIDER: &str = "openai";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentCodexProfile {
    pub provider: String,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStatus {
    pub ok: bool,
    pub ready: bool,
    pub reason: Option<String>,
    pub current_provider: String,
    pub current_model: Option<String>,
    pub total_threads: i64,
    pub mismatched_provider_threads: i64,
    pub mismatched_model_threads: Option<i64>,
    pub session_file_count: usize,
    pub session_index_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySyncResult {
    pub ok: bool,
    pub current_provider: String,
    pub current_model: Option<String>,
    pub updated_database_rows: usize,
    pub updated_session_files: usize,
    pub updated_session_index: bool,
    pub updated_global_state: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySyncOptions {
    pub custom_workspace_roots: Vec<String>,
}

pub fn read_current_profile(paths: &CodexPaths) -> Result<CurrentCodexProfile> {
    let text = fs::read_to_string(&paths.config_path)?;
    let doc = text.parse::<DocumentMut>()?;
    let provider = doc
        .get("model_provider")
        .and_then(|item| item.as_str())
        .unwrap_or(DEFAULT_PROVIDER)
        .trim()
        .to_string();
    let model = doc
        .get("model")
        .and_then(|item| item.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    Ok(CurrentCodexProfile { provider, model })
}

pub fn history_status(paths: &CodexPaths) -> Result<HistoryStatus> {
    let missing = missing_reason(paths);
    if let Some(reason) = missing {
        return Ok(HistoryStatus {
            ok: true,
            ready: false,
            reason: Some(reason),
            current_provider: String::new(),
            current_model: None,
            total_threads: 0,
            mismatched_provider_threads: 0,
            mismatched_model_threads: None,
            session_file_count: session_files(paths).len(),
            session_index_count: read_session_index_entries(paths).unwrap_or_default().len(),
        });
    }

    let profile = read_current_profile(paths)?;
    let conn =
        Connection::open_with_flags(&paths.db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    if !table_exists(&conn, "threads")? {
        return Ok(HistoryStatus {
            ok: true,
            ready: true,
            reason: Some("missing threads table".to_string()),
            current_provider: profile.provider,
            current_model: profile.model,
            total_threads: 0,
            mismatched_provider_threads: 0,
            mismatched_model_threads: None,
            session_file_count: session_files(paths).len(),
            session_index_count: read_session_index_entries(paths)?.len(),
        });
    }
    let columns = table_columns(&conn, "threads")?;
    let total_threads: i64 =
        conn.query_row("SELECT COUNT(*) FROM threads", [], |row| row.get(0))?;
    let mismatched_provider_threads: i64 = if columns.contains("model_provider") {
        conn.query_row(
            "SELECT COUNT(*) FROM threads WHERE model_provider IS NULL OR model_provider <> ?",
            [&profile.provider],
            |row| row.get(0),
        )?
    } else {
        0
    };
    Ok(HistoryStatus {
        ok: true,
        ready: true,
        reason: None,
        current_provider: profile.provider,
        current_model: profile.model,
        total_threads,
        mismatched_provider_threads,
        mismatched_model_threads: None,
        session_file_count: session_files(paths).len(),
        session_index_count: read_session_index_entries(paths)?.len(),
    })
}

pub fn sync_history(paths: &CodexPaths) -> Result<HistorySyncResult> {
    sync_history_with_options(paths, &HistorySyncOptions::default())
}

pub fn sync_history_with_options(
    paths: &CodexPaths,
    options: &HistorySyncOptions,
) -> Result<HistorySyncResult> {
    if let Some(reason) = missing_reason(paths) {
        return Err(RemoteCodexError::Message(reason));
    }
    let profile = read_current_profile(paths)?;
    let updated_database_rows = update_database_threads(paths, &profile)?;
    let updated_session_files = update_session_files(paths, &profile)?;
    let updated_session_index = merge_session_index(paths)?;
    let updated_global_state = sync_global_state(paths, &options.custom_workspace_roots)?;
    Ok(HistorySyncResult {
        ok: true,
        current_provider: profile.provider,
        current_model: profile.model,
        updated_database_rows,
        updated_session_files,
        updated_session_index,
        updated_global_state,
    })
}

fn missing_reason(paths: &CodexPaths) -> Option<String> {
    let mut missing = Vec::new();
    if !paths.config_path.exists() {
        missing.push(paths.config_path.display().to_string());
    }
    if !paths.db_path.exists() {
        missing.push(paths.db_path.display().to_string());
    }
    (!missing.is_empty()).then(|| format!("missing {}", missing.join(", ")))
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?",
            [table],
            |_| Ok(()),
        )
        .is_ok();
    Ok(exists)
}

fn table_columns(conn: &Connection, table: &str) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let mut columns = HashSet::new();
    for row in rows {
        columns.insert(row?);
    }
    Ok(columns)
}

fn update_database_threads(paths: &CodexPaths, profile: &CurrentCodexProfile) -> Result<usize> {
    let conn = Connection::open(&paths.db_path)?;
    conn.busy_timeout(std::time::Duration::from_secs(30))?;
    if !table_exists(&conn, "threads")? {
        return Ok(0);
    }
    let columns = table_columns(&conn, "threads")?;
    if !columns.contains("model_provider") {
        return Ok(0);
    }
    let changed = conn.execute(
        "UPDATE threads SET model_provider = ? WHERE model_provider IS NULL OR model_provider <> ?",
        (&profile.provider, &profile.provider),
    )?;
    Ok(changed)
}

fn update_session_files(paths: &CodexPaths, profile: &CurrentCodexProfile) -> Result<usize> {
    let mut changed = 0;
    for path in session_files(paths) {
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let Some((first_line, ending, remainder)) = split_first_line(&text) else {
            continue;
        };
        let mut item: Value = match serde_json::from_str(first_line) {
            Ok(item) => item,
            Err(_) => continue,
        };
        if item.get("type").and_then(Value::as_str) != Some("session_meta") {
            continue;
        }
        let Some(payload) = item.get_mut("payload").and_then(Value::as_object_mut) else {
            continue;
        };
        let current_provider = payload
            .get("model_provider")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if current_provider == profile.provider {
            continue;
        }
        payload.insert(
            "model_provider".to_string(),
            Value::String(profile.provider.clone()),
        );
        fs::write(
            path,
            format!("{}{}{}", serde_json::to_string(&item)?, ending, remainder),
        )?;
        changed += 1;
    }
    Ok(changed)
}

fn merge_session_index(paths: &CodexPaths) -> Result<bool> {
    let db_entries = active_thread_index_entries(paths)?;
    let existing_entries = read_session_index_entries(paths)?;
    let file_ids = session_file_thread_ids(paths);
    let existing_by_id = existing_entries
        .iter()
        .filter_map(|entry| {
            entry
                .get("id")
                .and_then(Value::as_str)
                .map(|id| (id.to_string(), entry.clone()))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let mut merged = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in db_entries {
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if id.is_empty() || !file_ids.contains(&id) {
            continue;
        }
        let mut merged_entry = existing_by_id
            .get(&id)
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        merge_object(&mut merged_entry, &entry);
        seen.insert(id);
        merged.push(merged_entry);
    }
    for entry in existing_entries {
        let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
        if !id.is_empty() && !seen.contains(id) && file_ids.contains(id) {
            seen.insert(id.to_string());
            merged.push(entry);
        }
    }
    let content = if merged.is_empty() {
        String::new()
    } else {
        merged
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join("\n")
            + "\n"
    };
    let existing = fs::read_to_string(&paths.session_index_path).unwrap_or_default();
    if content == existing {
        return Ok(false);
    }
    if let Some(parent) = paths.session_index_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&paths.session_index_path, content)?;
    Ok(true)
}

fn active_thread_index_entries(paths: &CodexPaths) -> Result<Vec<Value>> {
    let conn =
        Connection::open_with_flags(&paths.db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    if !table_exists(&conn, "threads")? {
        return Ok(Vec::new());
    }
    let columns = table_columns(&conn, "threads")?;
    let title_expr = if columns.contains("title") {
        "title"
    } else {
        "id AS title"
    };
    let rollout_expr = if columns.contains("rollout_path") {
        "rollout_path"
    } else {
        "'' AS rollout_path"
    };
    let updated_expr = if columns.contains("updated_at") {
        "updated_at"
    } else {
        "0 AS updated_at"
    };
    let updated_ms_expr = if columns.contains("updated_at_ms") {
        "updated_at_ms"
    } else {
        "0 AS updated_at_ms"
    };
    let where_sql = if columns.contains("archived") {
        "WHERE archived = 0"
    } else {
        ""
    };
    let order_sql = if columns.contains("updated_at_ms") || columns.contains("updated_at") {
        "ORDER BY COALESCE(updated_at_ms, updated_at * 1000, 0) ASC, id ASC"
    } else {
        "ORDER BY id ASC"
    };
    let sql = format!(
        "SELECT id, {title_expr}, {rollout_expr}, {updated_expr}, {updated_ms_expr} FROM threads {where_sql} {order_sql}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let title: Option<String> = row.get(1)?;
        let rollout_path: Option<String> = row.get(2)?;
        let updated_at: Option<i64> = row.get(3)?;
        let updated_at_ms: Option<i64> = row.get(4)?;
        Ok((id, title, rollout_path, updated_at, updated_at_ms))
    })?;
    let mut entries = Vec::new();
    for row in rows {
        let (id, title, rollout_path, updated_at, updated_at_ms) = row?;
        let rollout_ms = rollout_path
            .as_deref()
            .filter(|path| !path.is_empty())
            .and_then(|path| latest_session_timestamp_ms(Path::new(path)));
        let timestamp = rollout_ms
            .or(updated_at_ms)
            .or_else(|| updated_at.map(|value| value * 1000))
            .unwrap_or(0);
        entries.push(json_object(&[
            ("id", Value::String(id.clone())),
            (
                "thread_name",
                Value::String(title.filter(|value| !value.is_empty()).unwrap_or(id)),
            ),
            ("updated_at", Value::String(iso_from_ms(timestamp))),
        ]));
    }
    Ok(entries)
}

fn sync_global_state(paths: &CodexPaths, custom_workspace_roots: &[String]) -> Result<bool> {
    let file_ids = session_file_thread_ids(paths);
    let entries = active_thread_ui_entries(paths, &file_ids)?;
    let mut state = if paths.global_state_path.exists() {
        serde_json::from_str::<Value>(&fs::read_to_string(&paths.global_state_path)?)?
    } else {
        Value::Object(Map::new())
    };
    if !state.is_object() {
        state = Value::Object(Map::new());
    }
    let object = state.as_object_mut().expect("state object");
    object.remove("thread-workspace-root-hints");
    object.remove("project-order");
    object.remove("electron-saved-workspace-roots");
    object.remove("projectless-thread-ids");
    let mut hints = Map::new();
    let mut project_order = BTreeSet::new();
    let mut saved_roots = BTreeSet::new();
    let mut projectless = BTreeSet::new();

    for (thread_id, cwd) in entries {
        if cwd.trim().is_empty() {
            projectless.insert(thread_id);
            continue;
        }
        hints.insert(thread_id.clone(), Value::String(cwd.clone()));
        project_order.insert(cwd.clone());
        saved_roots.insert(cwd);
        projectless.remove(&thread_id);
    }
    for root in custom_workspace_roots {
        let root = root.trim().trim_end_matches(['/', '\\']);
        if root.is_empty() {
            continue;
        }
        project_order.insert(root.to_string());
        saved_roots.insert(root.to_string());
    }

    object.insert(
        "thread-workspace-root-hints".to_string(),
        Value::Object(hints),
    );
    object.insert(
        "project-order".to_string(),
        string_set_to_value(project_order),
    );
    object.insert(
        "electron-saved-workspace-roots".to_string(),
        string_set_to_value(saved_roots),
    );
    object.insert(
        "projectless-thread-ids".to_string(),
        string_set_to_value(projectless),
    );
    let new_content = serde_json::to_string_pretty(&state)? + "\n";
    let existing = fs::read_to_string(&paths.global_state_path).unwrap_or_default();
    if new_content == existing {
        return Ok(false);
    }
    fs::write(&paths.global_state_path, new_content)?;
    Ok(true)
}

fn active_thread_ui_entries(
    paths: &CodexPaths,
    valid_thread_ids: &HashSet<String>,
) -> Result<Vec<(String, String)>> {
    let conn =
        Connection::open_with_flags(&paths.db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    if !table_exists(&conn, "threads")? {
        return Ok(Vec::new());
    }
    let columns = table_columns(&conn, "threads")?;
    let cwd_expr = if columns.contains("cwd") {
        "cwd"
    } else {
        "'' AS cwd"
    };
    let where_sql = if columns.contains("archived") {
        "WHERE archived = 0"
    } else {
        ""
    };
    let sql = format!("SELECT id, {cwd_expr} FROM threads {where_sql} ORDER BY id ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        ))
    })?;
    let mut entries = Vec::new();
    for row in rows {
        let (id, cwd) = row?;
        if valid_thread_ids.contains(&id) {
            entries.push((id, cwd));
        }
    }
    Ok(entries)
}

fn read_session_index_entries(paths: &CodexPaths) -> Result<Vec<Value>> {
    if !paths.session_index_path.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for line in fs::read_to_string(&paths.session_index_path)?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            if value.get("id").is_some() {
                entries.push(value);
            }
        }
    }
    Ok(entries)
}

fn session_file_thread_ids(paths: &CodexPaths) -> HashSet<String> {
    let mut ids = HashSet::new();
    for path in session_files(paths) {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let Some((first, _, _)) = split_first_line(&text) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(first) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) == Some("session_meta") {
            if let Some(id) = value.pointer("/payload/id").and_then(Value::as_str) {
                ids.insert(id.to_string());
            }
        }
    }
    ids
}

fn session_files(paths: &CodexPaths) -> Vec<PathBuf> {
    if !paths.sessions_dir.exists() {
        return Vec::new();
    }
    let mut files = WalkDir::new(&paths.sessions_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry.file_name().to_string_lossy().starts_with("rollout-")
                && entry.path().extension().and_then(|ext| ext.to_str()) == Some("jsonl")
        })
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn latest_session_timestamp_ms(path: &Path) -> Option<i64> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines().rev() {
        let value = serde_json::from_str::<Value>(line).ok()?;
        if let Some(timestamp) = item_timestamp_ms(&value) {
            return Some(timestamp);
        }
    }
    None
}

fn item_timestamp_ms(value: &Value) -> Option<i64> {
    parse_timestamp_ms(value.get("timestamp")).or_else(|| {
        value
            .get("payload")
            .and_then(|payload| parse_timestamp_ms(payload.get("timestamp")))
    })
}

fn parse_timestamp_ms(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => {
            let raw = number.as_f64()?;
            (raw > 0.0).then_some(if raw > 10_000_000_000.0 {
                raw as i64
            } else {
                (raw * 1000.0) as i64
            })
        }
        Value::String(text) => {
            if let Ok(number) = text.parse::<f64>() {
                return (number > 0.0).then_some(if number > 10_000_000_000.0 {
                    number as i64
                } else {
                    (number * 1000.0) as i64
                });
            }
            DateTime::parse_from_rfc3339(text)
                .ok()
                .map(|dt| dt.timestamp_millis())
        }
        _ => None,
    }
}

fn split_first_line(text: &str) -> Option<(&str, &str, &str)> {
    for ending in ["\r\n", "\n", "\r"] {
        if let Some(index) = text.find(ending) {
            return Some((&text[..index], ending, &text[index + ending.len()..]));
        }
    }
    (!text.is_empty()).then_some((text, "\n", ""))
}

fn merge_object(target: &mut Value, patch: &Value) {
    let target = target.as_object_mut();
    let patch = patch.as_object();
    if let (Some(target), Some(patch)) = (target, patch) {
        for (key, value) in patch {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn json_object(entries: &[(&str, Value)]) -> Value {
    let mut object = Map::new();
    for (key, value) in entries {
        object.insert((*key).to_string(), value.clone());
    }
    Value::Object(object)
}

fn string_set_to_value(values: BTreeSet<String>) -> Value {
    Value::Array(values.into_iter().map(Value::String).collect())
}

fn iso_from_ms(timestamp_ms: i64) -> String {
    Utc.timestamp_millis_opt(timestamp_ms.max(0))
        .single()
        .unwrap_or_else(|| Utc.timestamp_millis_opt(0).single().expect("epoch"))
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}
