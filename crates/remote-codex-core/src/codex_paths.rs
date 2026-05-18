use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPaths {
    pub codex_home: PathBuf,
    pub config_path: PathBuf,
    pub auth_path: PathBuf,
    pub db_path: PathBuf,
    pub sessions_dir: PathBuf,
    pub session_index_path: PathBuf,
    pub global_state_path: PathBuf,
    pub backup_dir: PathBuf,
}

impl CodexPaths {
    pub fn from_home(home: impl AsRef<Path>) -> Self {
        let codex_home = home.as_ref().to_path_buf();
        Self {
            config_path: codex_home.join("config.toml"),
            auth_path: codex_home.join("auth.json"),
            db_path: codex_home.join("state_5.sqlite"),
            sessions_dir: codex_home.join("sessions"),
            session_index_path: codex_home.join("session_index.jsonl"),
            global_state_path: codex_home.join(".codex-global-state.json"),
            backup_dir: codex_home.join("codex_profile_tray_backups"),
            codex_home,
        }
    }

    pub fn resolve(codex_home: Option<&str>) -> Self {
        match codex_home.and_then(|value| (!value.trim().is_empty()).then_some(value.trim())) {
            Some(value) => Self::from_home(expand_path(value)),
            None => Self::from_home(
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".codex"),
            ),
        }
    }
}

fn expand_path(value: &str) -> PathBuf {
    let value = expand_env_vars(value);
    if value == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(&value));
    }
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    if let Some(rest) = value.strip_prefix("~\\") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(value)
}

fn expand_env_vars(value: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        let mut expanded = String::new();
        let mut rest = value;
        while let Some(start) = rest.find('%') {
            expanded.push_str(&rest[..start]);
            let after_start = &rest[start + 1..];
            let Some(end) = after_start.find('%') else {
                expanded.push('%');
                expanded.push_str(after_start);
                return expanded;
            };
            let key = &after_start[..end];
            if let Ok(var) = std::env::var(key) {
                expanded.push_str(&var);
            } else {
                expanded.push('%');
                expanded.push_str(key);
                expanded.push('%');
            }
            rest = &after_start[end + 1..];
        }
        expanded.push_str(rest);
        expanded
    }
    #[cfg(not(target_os = "windows"))]
    {
        value.to_string()
    }
}
