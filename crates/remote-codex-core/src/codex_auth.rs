use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

use crate::{RemoteCodexError, Result};

pub fn apply_chatgpt_auth(auth_path: &Path) -> Result<()> {
    let mut object = if auth_path.exists() {
        let value: Value = serde_json::from_str(&fs::read_to_string(auth_path)?)?;
        value.as_object().cloned().ok_or_else(|| {
            RemoteCodexError::Message("auth.json must be a JSON object".to_string())
        })?
    } else {
        Map::new()
    };
    object.insert(
        "auth_mode".to_string(),
        Value::String("chatgpt".to_string()),
    );
    object.insert("OPENAI_API_KEY".to_string(), Value::Null);
    if let Some(parent) = auth_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        auth_path,
        serde_json::to_string_pretty(&Value::Object(object))? + "\n",
    )?;
    Ok(())
}
