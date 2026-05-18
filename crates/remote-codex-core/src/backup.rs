use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use walkdir::WalkDir;

use crate::{CodexPaths, RemoteCodexError, Result};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRecord {
    pub id: String,
    pub label: String,
    pub created_at: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RolloutJournalEntry {
    path: String,
    first_line: String,
}

pub fn create_backup(paths: &CodexPaths, label: &str) -> Result<BackupRecord> {
    let created_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = format!(
        "{}-{}",
        created_at
            .replace([':', '-'], "")
            .replace('T', "-")
            .replace('Z', ""),
        sanitize_label(label)
    );
    let target = paths.backup_dir.join(&id);
    fs::create_dir_all(&target)?;

    copy_if_exists(&paths.config_path, &target.join("config.toml"))?;
    copy_if_exists(&paths.auth_path, &target.join("auth.json"))?;
    copy_if_exists(&paths.db_path, &target.join("state_5.sqlite"))?;
    copy_if_exists(
        &paths.session_index_path,
        &target.join("session_index.jsonl"),
    )?;
    copy_if_exists(
        &paths.global_state_path,
        &target.join(".codex-global-state.json"),
    )?;

    let rollout_journal = snapshot_rollout_first_lines(paths)?;
    fs::write(
        target.join("rollout-journal.json"),
        serde_json::to_string_pretty(&rollout_journal)? + "\n",
    )?;

    let record = BackupRecord {
        id,
        label: label.to_string(),
        created_at,
        path: target,
    };
    fs::write(
        record.path.join("manifest.json"),
        serde_json::to_string_pretty(&json!({
            "id": record.id,
            "label": record.label,
            "createdAt": record.created_at
        }))? + "\n",
    )?;
    Ok(record)
}

pub fn list_backups(paths: &CodexPaths) -> Result<Vec<BackupRecord>> {
    if !paths.backup_dir.exists() {
        return Ok(Vec::new());
    }
    let mut records = Vec::new();
    for entry in fs::read_dir(&paths.backup_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let manifest = entry.path().join("manifest.json");
        if !manifest.exists() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(manifest)?)?;
        records.push(BackupRecord {
            id: value["id"].as_str().unwrap_or_default().to_string(),
            label: value["label"].as_str().unwrap_or_default().to_string(),
            created_at: value["createdAt"].as_str().unwrap_or_default().to_string(),
            path: entry.path(),
        });
    }
    records.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(records)
}

pub fn restore_backup(paths: &CodexPaths, backup_id: &str) -> Result<()> {
    let backup_path = paths.backup_dir.join(backup_id);
    if !backup_path.exists() {
        return Err(RemoteCodexError::Message(format!(
            "backup not found: {backup_id}"
        )));
    }
    copy_if_exists(&backup_path.join("config.toml"), &paths.config_path)?;
    copy_if_exists(&backup_path.join("auth.json"), &paths.auth_path)?;
    copy_if_exists(&backup_path.join("state_5.sqlite"), &paths.db_path)?;
    copy_if_exists(
        &backup_path.join("session_index.jsonl"),
        &paths.session_index_path,
    )?;
    copy_if_exists(
        &backup_path.join(".codex-global-state.json"),
        &paths.global_state_path,
    )?;

    let journal_path = backup_path.join("rollout-journal.json");
    if journal_path.exists() {
        let entries: Vec<RolloutJournalEntry> =
            serde_json::from_str(&fs::read_to_string(journal_path)?)?;
        for entry in entries {
            let path = paths.codex_home.join(entry.path);
            restore_first_line(&path, &entry.first_line)?;
        }
    }
    Ok(())
}

fn copy_if_exists(source: &Path, target: &Path) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, target)?;
    Ok(())
}

fn snapshot_rollout_first_lines(paths: &CodexPaths) -> Result<Vec<RolloutJournalEntry>> {
    if !paths.sessions_dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in WalkDir::new(&paths.sessions_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file()
            || !entry.file_name().to_string_lossy().starts_with("rollout-")
        {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let text = fs::read_to_string(path)?;
        let first_line = text.lines().next().unwrap_or_default().to_string();
        let relative = path
            .strip_prefix(&paths.codex_home)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        entries.push(RolloutJournalEntry {
            path: relative,
            first_line,
        });
    }
    Ok(entries)
}

fn restore_first_line(path: &Path, first_line: &str) -> Result<()> {
    let remainder = if path.exists() {
        let text = fs::read_to_string(path)?;
        match split_first_line(&text) {
            Some((_, ending, rest)) => format!("{ending}{rest}"),
            None => "\n".to_string(),
        }
    } else {
        "\n".to_string()
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{first_line}{remainder}"))?;
    Ok(())
}

fn split_first_line(text: &str) -> Option<(&str, &str, &str)> {
    for ending in ["\r\n", "\n", "\r"] {
        if let Some(index) = text.find(ending) {
            return Some((&text[..index], ending, &text[index + ending.len()..]));
        }
    }
    (!text.is_empty()).then_some((text, "", ""))
}

fn sanitize_label(label: &str) -> String {
    let text: String = label
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect();
    if text.is_empty() {
        "backup".to_string()
    } else {
        text
    }
}
