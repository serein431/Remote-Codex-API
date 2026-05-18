use std::fs;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use toml_edit::{DocumentMut, Item};

use crate::{CodexPaths, Result};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexRuntimeStatus {
    pub auth_mode_chatgpt: bool,
    pub openai_api_key_null: bool,
    pub current_provider: String,
    pub current_model: Option<String>,
    pub provider_configured: bool,
    pub provider_name: Option<String>,
    pub provider_requires_openai_auth: bool,
    pub provider_has_bearer_token: bool,
    pub ready_for_remote: bool,
}

pub fn codex_runtime_status(paths: &CodexPaths) -> Result<CodexRuntimeStatus> {
    let auth = read_auth(paths)?;
    let config = read_config(paths)?;
    let current_provider = config
        .get("model_provider")
        .and_then(Item::as_str)
        .unwrap_or("")
        .to_string();
    let current_model = config
        .get("model")
        .and_then(Item::as_str)
        .map(str::to_string);
    let provider = config
        .get("model_providers")
        .and_then(Item::as_table)
        .and_then(|providers| providers.get(&current_provider))
        .and_then(Item::as_table);
    let provider_configured = provider.is_some();
    let provider_name = provider
        .and_then(|table| table.get("name"))
        .and_then(Item::as_str)
        .map(str::to_string);
    let provider_requires_openai_auth = provider
        .and_then(|table| table.get("requires_openai_auth"))
        .and_then(Item::as_bool)
        .unwrap_or(false);
    let provider_has_bearer_token = provider
        .and_then(|table| table.get("experimental_bearer_token"))
        .and_then(Item::as_str)
        .map(|token| !token.trim().is_empty())
        .unwrap_or(false);
    let auth_mode_chatgpt = auth
        .get("auth_mode")
        .and_then(Value::as_str)
        .map(|mode| mode == "chatgpt")
        .unwrap_or(false);
    let openai_api_key_null = auth
        .get("OPENAI_API_KEY")
        .map(Value::is_null)
        .unwrap_or(false);
    let ready_for_remote = auth_mode_chatgpt
        && openai_api_key_null
        && provider_configured
        && provider_requires_openai_auth
        && provider_has_bearer_token;

    Ok(CodexRuntimeStatus {
        auth_mode_chatgpt,
        openai_api_key_null,
        current_provider,
        current_model,
        provider_configured,
        provider_name,
        provider_requires_openai_auth,
        provider_has_bearer_token,
        ready_for_remote,
    })
}

fn read_auth(paths: &CodexPaths) -> Result<Value> {
    if !paths.auth_path.exists() {
        return Ok(Value::Object(Default::default()));
    }
    Ok(serde_json::from_str(&fs::read_to_string(
        &paths.auth_path,
    )?)?)
}

fn read_config(paths: &CodexPaths) -> Result<DocumentMut> {
    if !paths.config_path.exists() {
        return Ok(DocumentMut::new());
    }
    let text = fs::read_to_string(&paths.config_path)?;
    if text.trim().is_empty() {
        return Ok(DocumentMut::new());
    }
    Ok(text.parse::<DocumentMut>()?)
}
