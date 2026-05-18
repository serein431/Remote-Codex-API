pub mod backup;
pub mod codex_auth;
pub mod codex_config;
pub mod codex_paths;
pub mod codex_status;
pub mod history;
pub mod history_roots;
pub mod profile;
pub mod secrets;

pub use codex_paths::CodexPaths;
pub use profile::CodexProfile;

pub const CODEX_PROVIDER_BUCKET_ID: &str = "remote-codex-api";
pub const CODEX_PROVIDER_BUCKET_NAME: &str = "Remote Codex API";

pub type Result<T> = std::result::Result<T, RemoteCodexError>;

#[derive(Debug, thiserror::Error)]
pub enum RemoteCodexError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML error: {0}")]
    Toml(#[from] toml_edit::TomlError),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("keyring error: {0}")]
    Keyring(String),
    #[error("{0}")]
    Message(String),
}
