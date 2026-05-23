use std::fs;
use std::path::Path;

use toml_edit::{value, DocumentMut, Item, Table};

use crate::profile::{validate_profile, CodexProfile};
use crate::{Result, CODEX_PROVIDER_BUCKET_ID};

pub fn apply_provider_config(
    config_path: &Path,
    profile: &CodexProfile,
    token: &str,
) -> Result<()> {
    validate_profile(profile)?;
    let text = if config_path.exists() {
        fs::read_to_string(config_path)?
    } else {
        String::new()
    };
    let mut doc = if text.trim().is_empty() {
        DocumentMut::new()
    } else {
        text.parse::<DocumentMut>()?
    };

    doc["model_provider"] = value(CODEX_PROVIDER_BUCKET_ID);
    doc["model"] = value(profile.model.as_str());
    if !doc
        .get("model_providers")
        .map(Item::is_table)
        .unwrap_or(false)
    {
        doc["model_providers"] = Item::Table(Table::new());
    }
    let providers = doc["model_providers"]
        .as_table_mut()
        .expect("model_providers table");
    if !providers
        .get(CODEX_PROVIDER_BUCKET_ID)
        .map(Item::is_table)
        .unwrap_or(false)
    {
        providers.insert(CODEX_PROVIDER_BUCKET_ID, Item::Table(Table::new()));
    }
    let provider = providers
        .get_mut(CODEX_PROVIDER_BUCKET_ID)
        .and_then(Item::as_table_mut)
        .expect("provider table");
    provider["name"] = value(profile.provider_name.as_str());
    provider["base_url"] = value(profile.base_url.as_str());
    provider["wire_api"] = value("responses");
    provider["requires_openai_auth"] = value(profile.requires_openai_auth);
    provider["experimental_bearer_token"] = value(token);

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(config_path, doc.to_string())?;
    Ok(())
}

pub fn clear_remote_provider_config(config_path: &Path) -> Result<()> {
    let text = if config_path.exists() {
        fs::read_to_string(config_path)?
    } else {
        String::new()
    };
    let mut doc = if text.trim().is_empty() {
        DocumentMut::new()
    } else {
        text.parse::<DocumentMut>()?
    };

    if doc
        .get("model_provider")
        .and_then(Item::as_str)
        .map(|provider| provider == CODEX_PROVIDER_BUCKET_ID)
        .unwrap_or(false)
    {
        doc.remove("model_provider");
    }

    if let Some(providers) = doc.get_mut("model_providers").and_then(Item::as_table_mut) {
        providers.remove(CODEX_PROVIDER_BUCKET_ID);
        if providers.is_empty() {
            doc.remove("model_providers");
        }
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(config_path, doc.to_string())?;
    Ok(())
}
