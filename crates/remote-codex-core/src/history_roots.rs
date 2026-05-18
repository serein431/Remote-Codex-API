use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{RemoteCodexError, Result};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomHistoryRoot {
    pub id: String,
    pub label: String,
    pub path: String,
    pub created_at: String,
    pub updated_at: String,
}

impl CustomHistoryRoot {
    pub fn draft(label: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: String::new(),
            label: label.into(),
            path: path.into(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct HistoryRootStore {
    config_dir: PathBuf,
}

impl HistoryRootStore {
    pub fn default_store() -> Self {
        let base = dirs::config_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        Self::new(base.join("RemoteCodexAPI"))
    }

    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    pub fn roots_path(&self) -> PathBuf {
        self.config_dir.join("history_roots.json")
    }

    pub fn list_roots(&self) -> Result<Vec<CustomHistoryRoot>> {
        let path = self.roots_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(path)?;
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save_root(&self, root: CustomHistoryRoot) -> Result<CustomHistoryRoot> {
        let mut root = normalize_root(root)?;
        fs::create_dir_all(&self.config_dir)?;
        let mut roots = self.list_roots()?;
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let normalized_path = root_path_key(&root.path);
        let existing_index = roots
            .iter()
            .position(|item| item.id == root.id || root_path_key(&item.path) == normalized_path);

        if let Some(index) = existing_index {
            let existing = &roots[index];
            if root.id.is_empty() {
                root.id = existing.id.clone();
            }
            if root.created_at.trim().is_empty() {
                root.created_at = existing.created_at.clone();
            }
            root.updated_at = now;
            roots[index] = root.clone();
        } else {
            if root.id.is_empty() {
                root.id = unique_root_id(&roots, &root.path);
            }
            if root.created_at.trim().is_empty() {
                root.created_at = now.clone();
            }
            root.updated_at = now;
            roots.push(root.clone());
        }

        roots.sort_by(|left, right| {
            left.label
                .to_lowercase()
                .cmp(&right.label.to_lowercase())
                .then_with(|| left.path.to_lowercase().cmp(&right.path.to_lowercase()))
        });
        fs::write(
            self.roots_path(),
            serde_json::to_string_pretty(&roots)? + "\n",
        )?;
        Ok(root)
    }

    pub fn delete_root(&self, id: &str) -> Result<bool> {
        let mut roots = self.list_roots()?;
        let before = roots.len();
        roots.retain(|root| root.id != id);
        if roots.len() == before {
            return Ok(false);
        }
        fs::create_dir_all(&self.config_dir)?;
        fs::write(
            self.roots_path(),
            serde_json::to_string_pretty(&roots)? + "\n",
        )?;
        Ok(true)
    }
}

fn normalize_root(mut root: CustomHistoryRoot) -> Result<CustomHistoryRoot> {
    root.id = slugify(&root.id);
    root.label = root.label.trim().to_string();
    root.path = root.path.trim().trim_end_matches(['/', '\\']).to_string();
    if root.path.is_empty() {
        return Err(RemoteCodexError::Message(
            "history root path is required".to_string(),
        ));
    }
    if root.path.contains('\n') || root.path.contains('\r') {
        return Err(RemoteCodexError::Message(
            "history root path cannot contain newlines".to_string(),
        ));
    }
    if root.label.is_empty() {
        root.label = label_from_path(&root.path);
    }
    Ok(root)
}

fn unique_root_id(roots: &[CustomHistoryRoot], path: &str) -> String {
    let base = slugify(path);
    let base = if base.is_empty() {
        "workspace-root".to_string()
    } else {
        base
    };
    let occupied = roots
        .iter()
        .map(|root| root.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    if !occupied.contains(base.as_str()) {
        return base;
    }
    let mut index = 2;
    loop {
        let candidate = format!("{base}-{index}");
        if !occupied.contains(candidate.as_str()) {
            return candidate;
        }
        index += 1;
    }
}

fn label_from_path(path: &str) -> String {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Workspace")
        .to_string()
}

fn root_path_key(path: &str) -> String {
    path.trim()
        .trim_end_matches(['/', '\\'])
        .replace('\\', "/")
        .to_lowercase()
}

fn slugify(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(48)
        .collect()
}
