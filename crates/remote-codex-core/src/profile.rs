use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{RemoteCodexError, Result};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexProfile {
    pub id: String,
    pub name: String,
    pub provider_name: String,
    pub base_url: String,
    pub model: String,
    pub requires_openai_auth: bool,
    pub keep_chatgpt_login: bool,
    pub codex_home: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ProfileStore {
    config_dir: PathBuf,
}

impl ProfileStore {
    pub fn default_store() -> Self {
        let base = dirs::config_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        Self::new(base.join("RemoteCodexAPI"))
    }

    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    pub fn profiles_path(&self) -> PathBuf {
        self.config_dir.join("profiles.json")
    }

    pub fn list_profiles(&self) -> Result<Vec<CodexProfile>> {
        let path = self.profiles_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(path)?;
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save_profile(&self, profile: &CodexProfile) -> Result<()> {
        validate_profile(profile)?;
        fs::create_dir_all(&self.config_dir)?;
        let mut profiles = self.list_profiles()?;
        let mut profile = profile.clone();
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        if profile.created_at.trim().is_empty() {
            profile.created_at = now.clone();
        }
        profile.updated_at = if profile.updated_at.trim().is_empty() {
            now
        } else {
            profile.updated_at
        };
        match profiles.iter_mut().find(|item| item.id == profile.id) {
            Some(existing) => *existing = profile,
            None => profiles.push(profile),
        }
        profiles.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        fs::write(
            self.profiles_path(),
            serde_json::to_string_pretty(&profiles)? + "\n",
        )?;
        Ok(())
    }

    pub fn delete_profile(&self, id: &str) -> Result<bool> {
        let mut profiles = self.list_profiles()?;
        let before = profiles.len();
        profiles.retain(|profile| profile.id != id);
        if profiles.len() == before {
            return Ok(false);
        }
        fs::create_dir_all(&self.config_dir)?;
        fs::write(
            self.profiles_path(),
            serde_json::to_string_pretty(&profiles)? + "\n",
        )?;
        Ok(true)
    }
}

pub fn validate_profile(profile: &CodexProfile) -> Result<()> {
    if profile.id.trim().is_empty() {
        return Err(RemoteCodexError::Message(
            "profile id is required".to_string(),
        ));
    }
    if !profile
        .id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(RemoteCodexError::Message(
            "profile id may only contain ASCII letters, numbers, '_' and '-'".to_string(),
        ));
    }
    if profile.name.trim().is_empty() {
        return Err(RemoteCodexError::Message(
            "profile name is required".to_string(),
        ));
    }
    if profile.provider_name.trim().is_empty() {
        return Err(RemoteCodexError::Message(
            "provider name is required".to_string(),
        ));
    }
    if !(profile.base_url.starts_with("https://")
        || profile.base_url.starts_with("http://127.0.0.1"))
    {
        return Err(RemoteCodexError::Message(
            "base_url must be https:// or local 127.0.0.1".to_string(),
        ));
    }
    if profile.model.trim().is_empty() {
        return Err(RemoteCodexError::Message("model is required".to_string()));
    }
    Ok(())
}
