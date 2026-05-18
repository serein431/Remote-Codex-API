use std::fs;
use std::path::Path;

use remote_codex_core::backup::{create_backup, restore_backup};
use remote_codex_core::codex_auth::apply_chatgpt_auth;
use remote_codex_core::codex_config::apply_provider_config;
use remote_codex_core::codex_status::codex_runtime_status;
use remote_codex_core::history::{
    history_status, sync_history, sync_history_with_options, HistorySyncOptions,
};
use remote_codex_core::history_roots::{CustomHistoryRoot, HistoryRootStore};
use remote_codex_core::profile::{CodexProfile, ProfileStore};
use remote_codex_core::secrets::{InMemorySecretStore, SecretStore};
use remote_codex_core::{CodexPaths, CODEX_PROVIDER_BUCKET_ID};
use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;

fn profile(id: &str) -> CodexProfile {
    CodexProfile {
        id: id.to_string(),
        name: "JMRAI".to_string(),
        provider_name: "JMRAI".to_string(),
        base_url: "https://jmrai.net/v1".to_string(),
        model: "gpt-5.5".to_string(),
        requires_openai_auth: true,
        keep_chatgpt_login: true,
        codex_home: None,
        created_at: "2026-05-16T00:00:00Z".to_string(),
        updated_at: "2026-05-16T00:00:00Z".to_string(),
    }
}

fn write_config(home: &Path, provider: &str, model: &str) {
    fs::create_dir_all(home).unwrap();
    fs::write(
        home.join("config.toml"),
        format!(
            r#"# keep me
approval_policy = "never"
model_provider = "{provider}"
model = "{model}"

[model_providers.old]
name = "Old"
base_url = "https://old.example/v1"
"#
        ),
    )
    .unwrap();
}

fn create_threads_db(home: &Path) {
    let conn = Connection::open(home.join("state_5.sqlite")).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE threads (
            id TEXT PRIMARY KEY,
            title TEXT,
            rollout_path TEXT,
            cwd TEXT,
            updated_at INTEGER,
            updated_at_ms INTEGER,
            archived INTEGER DEFAULT 0,
            model_provider TEXT,
            model TEXT
        );
        INSERT INTO threads VALUES
          ('old-thread', 'Old Thread', '', '/work/project', 100, 100000, 0, 'old', 'gpt-old'),
          ('already-current', 'Current Thread', '', '', 200, 200000, 0, 'jmrai', 'gpt-5.5'),
          ('archived-thread', 'Archived', '', '/work/project', 300, 300000, 1, 'old', 'gpt-old');
        "#,
    )
    .unwrap();
}

fn write_session(home: &Path, id: &str, provider: &str, model: &str) {
    let dir = home.join("sessions/2026/05/16");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(format!("rollout-2026-05-16T00-00-00-{id}.jsonl")),
        format!(
            "{}\n{}\n",
            json!({
                "type": "session_meta",
                "payload": {
                    "id": id,
                    "model_provider": provider,
                    "model": model
                }
            }),
            json!({"type":"event_msg","timestamp":"2026-05-16T01:00:00Z"})
        ),
    )
    .unwrap();
}

#[test]
fn config_update_preserves_existing_toml_and_writes_provider_block() {
    let temp = TempDir::new().unwrap();
    let config = temp.path().join("config.toml");
    fs::write(&config, "approval_policy = \"never\"\n").unwrap();

    apply_provider_config(&config, &profile("jmrai"), "secret-token").unwrap();

    let text = fs::read_to_string(config).unwrap();
    assert!(text.contains("approval_policy = \"never\""));
    assert!(text.contains(&format!("model_provider = \"{CODEX_PROVIDER_BUCKET_ID}\"")));
    assert!(text.contains("model = \"gpt-5.5\""));
    assert!(text.contains(&format!("[model_providers.{CODEX_PROVIDER_BUCKET_ID}]")));
    assert!(text.contains("name = \"JMRAI\""));
    assert!(text.contains("base_url = \"https://jmrai.net/v1\""));
    assert!(text.contains("wire_api = \"responses\""));
    assert!(text.contains("requires_openai_auth = true"));
    assert!(text.contains("experimental_bearer_token = \"secret-token\""));
}

#[test]
fn config_update_uses_stable_codex_provider_bucket_for_all_profiles() {
    let temp = TempDir::new().unwrap();
    let config = temp.path().join("config.toml");

    apply_provider_config(&config, &profile("jmrai"), "secret-token").unwrap();
    let mut local = profile("local-proxy");
    local.name = "Local Proxy".to_string();
    local.provider_name = "Local Proxy".to_string();
    local.base_url = "http://127.0.0.1:48760/v1".to_string();
    local.model = "gpt-5.4".to_string();
    apply_provider_config(&config, &local, "local-token").unwrap();

    let text = fs::read_to_string(config).unwrap();
    assert!(text.contains(&format!("model_provider = \"{CODEX_PROVIDER_BUCKET_ID}\"")));
    assert!(text.contains(&format!("[model_providers.{CODEX_PROVIDER_BUCKET_ID}]")));
    assert!(!text.contains("[model_providers.jmrai]"));
    assert!(!text.contains("[model_providers.local-proxy]"));
    assert!(text.contains("name = \"Local Proxy\""));
    assert!(text.contains("base_url = \"http://127.0.0.1:48760/v1\""));
    assert!(text.contains("experimental_bearer_token = \"local-token\""));
}

#[test]
fn config_update_preserves_unknown_fields_inside_stable_provider_bucket() {
    let temp = TempDir::new().unwrap();
    let config = temp.path().join("config.toml");
    fs::write(
        &config,
        format!(
            r#"model_provider = "{CODEX_PROVIDER_BUCKET_ID}"

[model_providers.{CODEX_PROVIDER_BUCKET_ID}]
name = "Old Remote"
base_url = "https://old.example/v1"
custom_setting = "keep-me"
"#
        ),
    )
    .unwrap();

    apply_provider_config(&config, &profile("jmrai"), "secret-token").unwrap();

    let text = fs::read_to_string(config).unwrap();
    assert!(text.contains("name = \"JMRAI\""));
    assert!(text.contains("base_url = \"https://jmrai.net/v1\""));
    assert!(text.contains("custom_setting = \"keep-me\""));
}

#[test]
fn auth_update_only_forces_chatgpt_mode_and_null_api_key() {
    let temp = TempDir::new().unwrap();
    let auth = temp.path().join("auth.json");
    fs::write(
        &auth,
        serde_json::to_string_pretty(&json!({
            "auth_mode": "api_key",
            "OPENAI_API_KEY": "sk-live",
            "account_id": "keep"
        }))
        .unwrap(),
    )
    .unwrap();

    apply_chatgpt_auth(&auth).unwrap();

    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(auth).unwrap()).unwrap();
    assert_eq!(value["auth_mode"], "chatgpt");
    assert!(value["OPENAI_API_KEY"].is_null());
    assert_eq!(value["account_id"], "keep");
}

#[test]
fn runtime_status_reports_remote_unlock_requirements() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join(".codex");
    let paths = CodexPaths::from_home(&home);
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("auth.json"),
        r#"{"auth_mode":"chatgpt","OPENAI_API_KEY":null}"#,
    )
    .unwrap();
    apply_provider_config(&paths.config_path, &profile("jmrai"), "secret-token").unwrap();

    let status = codex_runtime_status(&paths).unwrap();

    assert!(status.auth_mode_chatgpt);
    assert!(status.openai_api_key_null);
    assert_eq!(status.current_provider, CODEX_PROVIDER_BUCKET_ID);
    assert!(status.provider_configured);
    assert!(status.provider_requires_openai_auth);
    assert!(status.provider_has_bearer_token);
    assert!(status.ready_for_remote);
}

#[test]
fn runtime_status_reports_missing_bearer_token_as_not_ready() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join(".codex");
    let paths = CodexPaths::from_home(&home);
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("auth.json"),
        r#"{"auth_mode":"chatgpt","OPENAI_API_KEY":null}"#,
    )
    .unwrap();
    fs::write(
        home.join("config.toml"),
        &format!(
            r#"model_provider = "{CODEX_PROVIDER_BUCKET_ID}"

[model_providers.{CODEX_PROVIDER_BUCKET_ID}]
name = "JMRAI"
base_url = "https://jmrai.net/v1"
requires_openai_auth = true
"#
        ),
    )
    .unwrap();

    let status = codex_runtime_status(&paths).unwrap();

    assert!(status.auth_mode_chatgpt);
    assert!(status.provider_requires_openai_auth);
    assert!(!status.provider_has_bearer_token);
    assert!(!status.ready_for_remote);
}

#[test]
fn profile_store_keeps_token_out_of_metadata_and_secret_store_keeps_it_separate() {
    let temp = TempDir::new().unwrap();
    let store = ProfileStore::new(temp.path().to_path_buf());
    let secrets = InMemorySecretStore::default();

    let profile = profile("jmrai");
    store.save_profile(&profile).unwrap();
    secrets.set_token(&profile.id, "secret-token").unwrap();

    let raw = fs::read_to_string(temp.path().join("profiles.json")).unwrap();
    assert!(raw.contains("JMRAI"));
    assert!(!raw.contains("secret-token"));
    assert_eq!(
        secrets.get_token("jmrai").unwrap().as_deref(),
        Some("secret-token")
    );
}

#[test]
fn history_status_is_read_only_and_sync_updates_all_local_visibility_files() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join(".codex");
    write_config(&home, "jmrai", "gpt-5.5");
    create_threads_db(&home);
    write_session(&home, "old-thread", "old", "gpt-old");
    fs::write(
        home.join("session_index.jsonl"),
        format!("{}\n", json!({"id":"already-current","thread_name":"Current Thread","updated_at":"1970-01-01T00:00:00Z"})),
    )
    .unwrap();
    fs::write(
        home.join(".codex-global-state.json"),
        serde_json::to_string_pretty(&json!({
            "thread-workspace-root-hints": {
                "empty-project": "/work/empty"
            },
            "project-order": ["/work/empty"],
            "electron-saved-workspace-roots": ["/work/empty"],
            "projectless-thread-ids": ["empty-project"]
        }))
        .unwrap(),
    )
    .unwrap();
    let paths = CodexPaths::from_home(&home);

    let before = history_status(&paths).unwrap();
    assert_eq!(before.mismatched_provider_threads, 2);
    assert!(!paths.backup_dir.exists());

    let result = sync_history(&paths).unwrap();
    assert_eq!(result.updated_database_rows, 2);
    assert_eq!(result.updated_session_files, 1);
    assert_eq!(result.updated_session_index, true);
    assert_eq!(result.updated_global_state, true);

    let after = history_status(&paths).unwrap();
    assert_eq!(after.mismatched_provider_threads, 0);
    assert_eq!(after.mismatched_model_threads, None);

    let first_line = fs::read_to_string(
        home.join("sessions/2026/05/16/rollout-2026-05-16T00-00-00-old-thread.jsonl"),
    )
    .unwrap()
    .lines()
    .next()
    .unwrap()
    .to_string();
    assert!(first_line.contains("\"model_provider\":\"jmrai\""));
    assert!(first_line.contains("\"model\":\"gpt-old\""));

    let index = fs::read_to_string(home.join("session_index.jsonl")).unwrap();
    assert!(index.contains("\"id\":\"old-thread\""));
    assert!(!index.contains("\"id\":\"already-current\""));

    let global_state = fs::read_to_string(home.join(".codex-global-state.json")).unwrap();
    assert!(global_state.contains("thread-workspace-root-hints"));
    assert!(global_state.contains("/work/project"));
    assert!(!global_state.contains("/work/empty"));
    assert!(!global_state.contains("empty-project"));
}

#[test]
fn history_sync_skips_database_threads_without_session_files() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join(".codex");
    write_config(&home, CODEX_PROVIDER_BUCKET_ID, "gpt-5.4");
    fs::create_dir_all(&home).unwrap();
    let conn = Connection::open(home.join("state_5.sqlite")).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE threads (
            id TEXT PRIMARY KEY,
            title TEXT,
            rollout_path TEXT,
            cwd TEXT,
            updated_at INTEGER,
            updated_at_ms INTEGER,
            archived INTEGER DEFAULT 0,
            model_provider TEXT,
            model TEXT
        );
        INSERT INTO threads VALUES
          ('real-thread', 'Real Thread', '', '/work/real', 100, 100000, 0, 'old', 'gpt-old'),
          ('empty-project', 'Empty Project', '', '/work/empty', 200, 200000, 0, 'old', 'gpt-old');
        "#,
    )
    .unwrap();
    write_session(&home, "real-thread", "old", "gpt-old");
    fs::write(
        home.join("session_index.jsonl"),
        format!(
            "{}\n",
            json!({"id":"empty-project","thread_name":"Empty Project","updated_at":"1970-01-01T00:00:00Z"})
        ),
    )
    .unwrap();
    fs::write(
        home.join(".codex-global-state.json"),
        serde_json::to_string_pretty(&json!({
            "thread-workspace-root-hints": {
                "empty-project": "/work/empty"
            },
            "project-order": ["/work/empty"],
            "electron-saved-workspace-roots": ["/work/empty"],
            "projectless-thread-ids": ["empty-project"]
        }))
        .unwrap(),
    )
    .unwrap();
    let paths = CodexPaths::from_home(&home);

    let result = sync_history(&paths).unwrap();

    assert!(result.updated_session_index);
    assert!(result.updated_global_state);
    let index = fs::read_to_string(home.join("session_index.jsonl")).unwrap();
    assert!(index.contains("\"id\":\"real-thread\""));
    assert!(!index.contains("\"id\":\"empty-project\""));

    let global_state = fs::read_to_string(home.join(".codex-global-state.json")).unwrap();
    assert!(global_state.contains("/work/real"));
    assert!(!global_state.contains("/work/empty"));
    assert!(!global_state.contains("empty-project"));
}

#[test]
fn history_sync_includes_user_custom_workspace_roots() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join(".codex");
    write_config(&home, CODEX_PROVIDER_BUCKET_ID, "gpt-5.4");
    fs::create_dir_all(&home).unwrap();
    let conn = Connection::open(home.join("state_5.sqlite")).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE threads (
            id TEXT PRIMARY KEY,
            title TEXT,
            rollout_path TEXT,
            cwd TEXT,
            updated_at INTEGER,
            updated_at_ms INTEGER,
            archived INTEGER DEFAULT 0,
            model_provider TEXT,
            model TEXT
        );
        INSERT INTO threads VALUES
          ('real-thread', 'Real Thread', '', '/work/real', 100, 100000, 0, 'old', 'gpt-old');
        "#,
    )
    .unwrap();
    write_session(&home, "real-thread", "old", "gpt-old");
    fs::write(home.join("session_index.jsonl"), "").unwrap();
    fs::write(home.join(".codex-global-state.json"), "{}").unwrap();
    let paths = CodexPaths::from_home(&home);

    sync_history_with_options(
        &paths,
        &HistorySyncOptions {
            custom_workspace_roots: vec![
                r"\\wsl.localhost\Ubuntu\home\dgsp\project".to_string(),
                "/Users/dgsp/Documents/custom".to_string(),
            ],
        },
    )
    .unwrap();

    let global_state = fs::read_to_string(home.join(".codex-global-state.json")).unwrap();
    assert!(global_state.contains(r"\\\\wsl.localhost\\Ubuntu\\home\\dgsp\\project"));
    assert!(global_state.contains("/Users/dgsp/Documents/custom"));
    assert!(global_state.contains("/work/real"));
}

#[test]
fn history_root_store_dedupes_roots_by_normalized_path() {
    let temp = TempDir::new().unwrap();
    let store = HistoryRootStore::new(temp.path().to_path_buf());

    let first = store
        .save_root(CustomHistoryRoot::draft(
            "WSL Ubuntu",
            r"\\wsl.localhost\Ubuntu\home\dgsp\project\",
        ))
        .unwrap();
    let second = store
        .save_root(CustomHistoryRoot::draft(
            "WSL Ubuntu renamed",
            r"\\WSL.LOCALHOST\Ubuntu\home\dgsp\project",
        ))
        .unwrap();
    let roots = store.list_roots().unwrap();

    assert_eq!(roots.len(), 1);
    assert_eq!(first.id, second.id);
    assert_eq!(roots[0].label, "WSL Ubuntu renamed");
    assert_eq!(roots[0].path, r"\\WSL.LOCALHOST\Ubuntu\home\dgsp\project");
}

#[test]
fn history_status_and_sync_ignore_model_differences() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join(".codex");
    write_config(&home, CODEX_PROVIDER_BUCKET_ID, "gpt-5.4");
    create_threads_db(&home);
    let conn = Connection::open(home.join("state_5.sqlite")).unwrap();
    conn.execute(
        "UPDATE threads SET model_provider = ?, model = ? WHERE id = 'already-current'",
        (CODEX_PROVIDER_BUCKET_ID, "gpt-5.5"),
    )
    .unwrap();
    drop(conn);
    write_session(&home, "old-thread", "old", "gpt-old");
    write_session(
        &home,
        "already-current",
        CODEX_PROVIDER_BUCKET_ID,
        "gpt-5.5",
    );
    fs::write(home.join("session_index.jsonl"), "").unwrap();
    fs::write(home.join(".codex-global-state.json"), "{}").unwrap();
    let paths = CodexPaths::from_home(&home);

    let before = history_status(&paths).unwrap();
    assert_eq!(before.mismatched_provider_threads, 2);
    assert_eq!(before.mismatched_model_threads, None);

    let result = sync_history(&paths).unwrap();
    assert_eq!(result.updated_database_rows, 2);
    assert_eq!(result.updated_session_files, 1);

    let conn = Connection::open(home.join("state_5.sqlite")).unwrap();
    let model: String = conn
        .query_row(
            "SELECT model FROM threads WHERE id = 'already-current'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(model, "gpt-5.5");

    let changed_rollout = fs::read_to_string(
        home.join("sessions/2026/05/16/rollout-2026-05-16T00-00-00-old-thread.jsonl"),
    )
    .unwrap();
    let changed_first_line = changed_rollout.lines().next().unwrap();
    assert!(changed_first_line.contains(&format!(
        "\"model_provider\":\"{CODEX_PROVIDER_BUCKET_ID}\""
    )));
    assert!(changed_first_line.contains("\"model\":\"gpt-old\""));

    let rollout = fs::read_to_string(
        home.join("sessions/2026/05/16/rollout-2026-05-16T00-00-00-already-current.jsonl"),
    )
    .unwrap();
    let first_line = rollout.lines().next().unwrap();
    assert!(first_line.contains(&format!(
        "\"model_provider\":\"{CODEX_PROVIDER_BUCKET_ID}\""
    )));
    assert!(first_line.contains("\"model\":\"gpt-5.5\""));
}

#[test]
fn backup_restore_returns_codex_files_and_rollout_metadata_to_previous_state() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join(".codex");
    write_config(&home, "old", "gpt-old");
    fs::write(
        home.join("auth.json"),
        r#"{"auth_mode":"api_key","OPENAI_API_KEY":"sk-old"}"#,
    )
    .unwrap();
    create_threads_db(&home);
    write_session(&home, "old-thread", "old", "gpt-old");
    fs::write(home.join("session_index.jsonl"), "{}\n").unwrap();
    fs::write(home.join(".codex-global-state.json"), "{}").unwrap();
    let paths = CodexPaths::from_home(&home);

    let backup = create_backup(&paths, "pre-activate").unwrap();
    apply_chatgpt_auth(&paths.auth_path).unwrap();
    apply_provider_config(&paths.config_path, &profile("jmrai"), "secret-token").unwrap();
    sync_history(&paths).unwrap();

    restore_backup(&paths, &backup.id).unwrap();

    let config = fs::read_to_string(home.join("config.toml")).unwrap();
    assert!(config.contains("model_provider = \"old\""));
    assert!(!config.contains("secret-token"));
    let auth = fs::read_to_string(home.join("auth.json")).unwrap();
    assert!(auth.contains("api_key"));
    assert!(auth.contains("sk-old"));
    let rollout = fs::read_to_string(
        home.join("sessions/2026/05/16/rollout-2026-05-16T00-00-00-old-thread.jsonl"),
    )
    .unwrap();
    assert!(rollout
        .lines()
        .next()
        .unwrap()
        .contains("\"model_provider\":\"old\""));
}
