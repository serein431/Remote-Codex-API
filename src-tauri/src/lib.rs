use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

#[cfg(target_os = "windows")]
use std::collections::HashSet;
#[cfg(target_os = "windows")]
use std::fs;

use remote_codex_core::backup::{
    create_backup, list_backups as core_list_backups, restore_backup as core_restore_backup,
    BackupRecord,
};
use remote_codex_core::codex_auth::apply_chatgpt_auth;
use remote_codex_core::codex_config::{apply_provider_config, clear_remote_provider_config};
use remote_codex_core::codex_status::{
    codex_runtime_status as core_codex_runtime_status, CodexRuntimeStatus,
};
use remote_codex_core::history::{
    history_status as core_history_status, sync_history_with_options as core_sync_history,
    HistoryStatus, HistorySyncOptions, HistorySyncResult,
};
use remote_codex_core::history_roots::{CustomHistoryRoot, HistoryRootStore};
use remote_codex_core::profile::{CodexProfile, ProfileStore};
use remote_codex_core::secrets::{KeyringSecretStore, SecretStore};
use remote_codex_core::CodexPaths;
use serde::{Deserialize, Serialize};
use tauri::menu::MenuBuilder;
use tauri::tray::TrayIconBuilder;
use tauri::Manager;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivationOptions {
    sync_history: bool,
    restart_codex: bool,
    token: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClearApiModeOptions {
    restart_codex: bool,
    codex_home: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivationResult {
    ok: bool,
    profile_id: String,
    backup_id: String,
    history: Option<HistorySyncResult>,
    codex_opened: bool,
    runtime_status: CodexRuntimeStatus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClearApiModeResult {
    ok: bool,
    backup_id: String,
    codex_opened: bool,
    runtime_status: CodexRuntimeStatus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsReport {
    app_version: String,
    platform: String,
    codex_home: String,
    config_path: String,
    auth_path: String,
    database_path: String,
    sessions_path: String,
    session_index_path: String,
    global_state_path: String,
    config_exists: bool,
    auth_exists: bool,
    database_exists: bool,
    sessions_exists: bool,
    session_index_exists: bool,
    global_state_exists: bool,
    backup_count: usize,
    profile_count: usize,
    custom_root_count: usize,
    codex_install_found: bool,
    codex_install_candidates: Vec<String>,
    codex_process_count: usize,
    runtime_status: Option<CodexRuntimeStatus>,
    runtime_error: Option<String>,
    history_status: Option<HistoryStatus>,
    history_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredProfile {
    #[serde(flatten)]
    profile: CodexProfile,
    token_ready: bool,
}

fn store() -> ProfileStore {
    ProfileStore::default_store()
}

fn history_roots() -> HistoryRootStore {
    HistoryRootStore::default_store()
}

fn secrets() -> KeyringSecretStore {
    KeyringSecretStore
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[tauri::command]
fn list_profiles() -> Result<Vec<StoredProfile>, String> {
    load_profiles().map_err(err)
}

#[tauri::command]
fn save_profile(
    profile: CodexProfile,
    token: Option<String>,
) -> Result<Vec<StoredProfile>, String> {
    store().save_profile(&profile).map_err(err)?;
    if let Some(token) = token.filter(|token| !token.trim().is_empty()) {
        secrets()
            .set_token(&profile.id, token.trim())
            .map_err(err)?;
    }
    load_profiles().map_err(err)
}

#[tauri::command]
fn delete_profile(id: String) -> Result<Vec<StoredProfile>, String> {
    store().delete_profile(&id).map_err(err)?;
    secrets().delete_token(&id).map_err(err)?;
    load_profiles().map_err(err)
}

#[tauri::command]
fn activate_profile(id: String, options: ActivationOptions) -> Result<ActivationResult, String> {
    let profile = store()
        .list_profiles()
        .map_err(err)?
        .into_iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| format!("profile not found: {id}"))?;
    let supplied_token = options
        .token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string);
    if let Some(token) = supplied_token.as_deref() {
        secrets().set_token(&profile.id, token).map_err(err)?;
    }
    let token = match supplied_token {
        Some(token) => token,
        None => secrets()
            .get_token(&profile.id)
            .map_err(err)?
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| format!("missing provider token for {}", profile.name))?,
    };
    let paths = CodexPaths::resolve(profile.codex_home.as_deref());
    let backup = create_backup(&paths, "pre-activate").map_err(err)?;
    apply_chatgpt_auth(&paths.auth_path).map_err(err)?;
    apply_provider_config(&paths.config_path, &profile, &token).map_err(err)?;
    let runtime_status = core_codex_runtime_status(&paths).map_err(err)?;
    let history = if options.sync_history {
        Some(core_sync_history(&paths, &history_sync_options()).map_err(err)?)
    } else {
        None
    };
    let codex_opened = if options.restart_codex {
        restart_codex().is_ok()
    } else {
        open_codex().is_ok()
    };
    Ok(ActivationResult {
        ok: true,
        profile_id: profile.id,
        backup_id: backup.id,
        history,
        codex_opened,
        runtime_status,
    })
}

#[tauri::command]
fn clear_api_mode(options: ClearApiModeOptions) -> Result<ClearApiModeResult, String> {
    let paths = CodexPaths::resolve(options.codex_home.as_deref());
    let backup = create_backup(&paths, "pre-clear-api-mode").map_err(err)?;
    apply_chatgpt_auth(&paths.auth_path).map_err(err)?;
    clear_remote_provider_config(&paths.config_path).map_err(err)?;
    let runtime_status = core_codex_runtime_status(&paths).map_err(err)?;
    let codex_opened = if options.restart_codex {
        restart_codex().is_ok()
    } else {
        open_codex().is_ok()
    };
    Ok(ClearApiModeResult {
        ok: true,
        backup_id: backup.id,
        codex_opened,
        runtime_status,
    })
}

#[tauri::command]
fn history_status(codex_home: Option<String>) -> Result<HistoryStatus, String> {
    core_history_status(&CodexPaths::resolve(codex_home.as_deref())).map_err(err)
}

#[tauri::command]
fn codex_status(codex_home: Option<String>) -> Result<CodexRuntimeStatus, String> {
    core_codex_runtime_status(&CodexPaths::resolve(codex_home.as_deref())).map_err(err)
}

#[tauri::command]
fn history_sync(codex_home: Option<String>) -> Result<HistorySyncResult, String> {
    let paths = CodexPaths::resolve(codex_home.as_deref());
    create_backup(&paths, "manual-history-sync").map_err(err)?;
    core_sync_history(&paths, &history_sync_options()).map_err(err)
}

#[tauri::command]
fn list_history_roots() -> Result<Vec<CustomHistoryRoot>, String> {
    history_roots().list_roots().map_err(err)
}

#[tauri::command]
fn save_history_root(
    path: String,
    label: Option<String>,
) -> Result<Vec<CustomHistoryRoot>, String> {
    let label = label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(path.trim())
        .to_string();
    history_roots()
        .save_root(CustomHistoryRoot::draft(label, path))
        .map_err(err)?;
    history_roots().list_roots().map_err(err)
}

#[tauri::command]
fn delete_history_root(id: String) -> Result<Vec<CustomHistoryRoot>, String> {
    history_roots().delete_root(&id).map_err(err)?;
    history_roots().list_roots().map_err(err)
}

#[tauri::command]
fn list_backups() -> Result<Vec<BackupRecord>, String> {
    core_list_backups(&CodexPaths::resolve(None)).map_err(err)
}

#[tauri::command]
fn restore_backup(id: String) -> Result<Vec<BackupRecord>, String> {
    let paths = CodexPaths::resolve(None);
    core_restore_backup(&paths, &id).map_err(err)?;
    core_list_backups(&paths).map_err(err)
}

#[tauri::command]
fn open_codex() -> Result<(), String> {
    open_codex_app().map_err(err)
}

#[tauri::command]
fn diagnostics(codex_home: Option<String>) -> DiagnosticsReport {
    let paths = CodexPaths::resolve(codex_home.as_deref());
    let backups = core_list_backups(&paths).unwrap_or_default();
    let profiles = store().list_profiles().unwrap_or_default();
    let roots = history_roots().list_roots().unwrap_or_default();
    let runtime = core_codex_runtime_status(&paths);
    let history = core_history_status(&paths);
    let candidates = codex_install_candidates_for_platform();
    DiagnosticsReport {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        codex_home: paths.codex_home.display().to_string(),
        config_path: paths.config_path.display().to_string(),
        auth_path: paths.auth_path.display().to_string(),
        database_path: paths.db_path.display().to_string(),
        sessions_path: paths.sessions_dir.display().to_string(),
        session_index_path: paths.session_index_path.display().to_string(),
        global_state_path: paths.global_state_path.display().to_string(),
        config_exists: paths.config_path.exists(),
        auth_exists: paths.auth_path.exists(),
        database_exists: paths.db_path.exists(),
        sessions_exists: paths.sessions_dir.exists(),
        session_index_exists: paths.session_index_path.exists(),
        global_state_exists: paths.global_state_path.exists(),
        backup_count: backups.len(),
        profile_count: profiles.len(),
        custom_root_count: roots.len(),
        codex_install_found: candidates.iter().any(|path| path.exists()),
        codex_install_candidates: candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        codex_process_count: codex_process_count(),
        runtime_status: runtime.as_ref().ok().cloned(),
        runtime_error: runtime.err().map(|error| error.to_string()),
        history_status: history.as_ref().ok().cloned(),
        history_error: history.err().map(|error| error.to_string()),
    }
}

fn load_profiles() -> remote_codex_core::Result<Vec<StoredProfile>> {
    hydrate_profiles(store().list_profiles()?, &secrets())
}

fn hydrate_profiles<S: SecretStore>(
    profiles: Vec<CodexProfile>,
    secrets: &S,
) -> remote_codex_core::Result<Vec<StoredProfile>> {
    profiles
        .into_iter()
        .map(|profile| {
            let token_ready = secrets
                .get_token(&profile.id)?
                .map(|token| !token.trim().is_empty())
                .unwrap_or(false);
            Ok(StoredProfile {
                profile,
                token_ready,
            })
        })
        .collect()
}

fn history_sync_options() -> HistorySyncOptions {
    let custom_workspace_roots = history_roots()
        .list_roots()
        .unwrap_or_default()
        .into_iter()
        .map(|root| root.path)
        .collect();
    HistorySyncOptions {
        custom_workspace_roots,
    }
}

fn restart_codex() -> Result<(), String> {
    stop_codex_processes()?;
    open_codex_app()
}

fn open_codex_app() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        for candidate in codex_macos_candidates() {
            if candidate.exists() {
                Command::new("open")
                    .arg(candidate)
                    .spawn()
                    .map_err(|error| format!("failed to open Codex: {error}"))?;
                return Ok(());
            }
        }
        Command::new("open")
            .arg("-a")
            .arg("Codex")
            .spawn()
            .map_err(|error| format!("failed to open Codex via LaunchServices: {error}"))?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        let mut launch_failures = Vec::new();
        for candidate in codex_windows_candidates() {
            if candidate.exists() {
                match Command::new(&candidate).spawn() {
                    Ok(_) => return Ok(()),
                    Err(error) => {
                        launch_failures.push(format!("{} ({error})", candidate.display()));
                    }
                }
            }
        }
        for command in ["codex.exe", "codex"] {
            match Command::new(command).spawn() {
                Ok(_) => return Ok(()),
                Err(error) => launch_failures.push(format!("{command} ({error})")),
            }
        }
        let searched = codex_windows_candidates()
            .into_iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let failures = if launch_failures.is_empty() {
            String::new()
        } else {
            format!(" Launch failures: {}.", launch_failures.join("; "))
        };
        return Err(format!(
            "Codex executable could not be opened. Install Codex or ensure codex.exe is on PATH.{failures} Searched: {searched}"
        ));
    }
    #[allow(unreachable_code)]
    Err("Remote Codex API currently supports launching Codex on macOS and Windows".to_string())
}

fn codex_install_candidates_for_platform() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        return codex_macos_candidates();
    }
    #[cfg(target_os = "windows")]
    {
        return codex_windows_candidates();
    }
    #[allow(unreachable_code)]
    Vec::new()
}

fn codex_process_count() -> usize {
    #[cfg(target_os = "macos")]
    {
        return codex_process_ids().map(|pids| pids.len()).unwrap_or(0);
    }
    #[cfg(target_os = "windows")]
    {
        return windows_codex_process_count();
    }
    #[allow(unreachable_code)]
    0
}

fn stop_codex_processes() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let pids = codex_process_ids()?;
        if !pids.is_empty() {
            Command::new("kill")
                .args(["-TERM"])
                .args(&pids)
                .status()
                .map_err(|error| format!("failed to stop Codex: {error}"))?;
            for _ in 0..30 {
                thread::sleep(Duration::from_millis(150));
                if codex_process_ids()?.is_empty() {
                    return Ok(());
                }
            }
            let remaining = codex_process_ids()?;
            if !remaining.is_empty() {
                Command::new("kill")
                    .args(["-KILL"])
                    .args(&remaining)
                    .status()
                    .map_err(|error| format!("failed to force stop Codex: {error}"))?;
            }
        }
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        for image in ["codex.exe", "Codex.exe"] {
            Command::new("taskkill")
                .args(["/IM", image, "/T", "/F"])
                .status()
                .map_err(|error| format!("failed to stop {image}: {error}"))?;
        }
        return Ok(());
    }
    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(target_os = "macos")]
fn codex_process_ids() -> Result<Vec<String>, String> {
    let output = Command::new("pgrep")
        .args(["-f", "/Codex.app/Contents/MacOS/Codex"])
        .output()
        .map_err(|error| format!("failed to inspect Codex processes: {error}"))?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<i32>().ok())
        .map(|pid| pid.to_string())
        .collect())
}

#[cfg(target_os = "macos")]
fn codex_macos_candidates() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/Applications/Codex.app"),
        PathBuf::from("/Applications/OpenAI Codex.app"),
    ];
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join("Applications/Codex.app"));
        paths.push(home.join("Applications/OpenAI Codex.app"));
    }
    paths
}

#[cfg(target_os = "windows")]
fn codex_windows_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    paths.extend(windows_path_candidates("codex.exe"));
    push_windows_candidates(
        &mut paths,
        "LOCALAPPDATA",
        &[
            r"OpenAI\Codex\bin\codex.exe",
            r"OpenAI\Codex\codex.exe",
            r"Programs\Codex\Codex.exe",
            r"Programs\Codex\codex.exe",
            r"Programs\OpenAI Codex\Codex.exe",
            r"Programs\OpenAI Codex\codex.exe",
            r"Codex\Codex.exe",
            r"Codex\codex.exe",
            r"OpenAI Codex\Codex.exe",
            r"OpenAI Codex\codex.exe",
        ],
    );
    push_windows_candidates(
        &mut paths,
        "PROGRAMFILES",
        &[r"Codex\Codex.exe", r"OpenAI Codex\Codex.exe"],
    );
    push_windows_candidates(
        &mut paths,
        "PROGRAMFILES(X86)",
        &[r"Codex\Codex.exe", r"OpenAI Codex\Codex.exe"],
    );
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(r"AppData\Local\Programs\Codex\Codex.exe"));
        paths.push(home.join(r"AppData\Local\Programs\Codex\codex.exe"));
        paths.push(home.join(r"AppData\Local\Programs\OpenAI Codex\Codex.exe"));
        paths.push(home.join(r"AppData\Local\Programs\OpenAI Codex\codex.exe"));
        paths.push(home.join(r"AppData\Local\OpenAI\Codex\bin\codex.exe"));
    }
    paths.extend(windows_store_codex_candidates());
    dedupe_paths(paths)
}

#[cfg(target_os = "windows")]
fn push_windows_candidates(paths: &mut Vec<PathBuf>, env_var: &str, relatives: &[&str]) {
    let Some(base) = std::env::var_os(env_var).map(PathBuf::from) else {
        return;
    };
    for relative in relatives {
        paths.push(base.join(relative));
    }
}

#[cfg(target_os = "windows")]
fn windows_path_candidates(command: &str) -> Vec<PathBuf> {
    let Ok(output) = Command::new("where.exe").arg(command).output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

#[cfg(target_os = "windows")]
fn windows_store_codex_candidates() -> Vec<PathBuf> {
    let Some(program_files) = std::env::var_os("PROGRAMFILES").map(PathBuf::from) else {
        return Vec::new();
    };
    let windows_apps = program_files.join("WindowsApps");
    let Ok(entries) = fs::read_dir(windows_apps) else {
        return Vec::new();
    };
    let mut packages = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("OpenAI.Codex_")
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    packages.sort_by(|left, right| right.cmp(left));

    let mut paths = Vec::new();
    for package in packages {
        paths.push(package.join(r"app\resources\codex.exe"));
        paths.push(package.join(r"app\codex.exe"));
        paths.push(package.join(r"app\Codex.exe"));
        paths.push(package.join("codex.exe"));
    }
    paths
}

#[cfg(target_os = "windows")]
fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.to_string_lossy().to_lowercase()))
        .collect()
}

#[cfg(target_os = "windows")]
fn windows_codex_process_count() -> usize {
    let Ok(output) = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq codex.exe", "/FO", "CSV", "/NH"])
        .output()
    else {
        return 0;
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.to_lowercase().contains("codex.exe"))
        .count()
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn install_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text("open-panel", "Open Remote Codex API")
        .separator()
        .text("sync-history", "Sync Local History")
        .text("open-codex", "Open Codex")
        .separator()
        .text("quit", "Quit")
        .build()?;

    let mut builder = TrayIconBuilder::with_id("remote-codex-api")
        .menu(&menu)
        .tooltip("Remote Codex API")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open-panel" => show_main_window(app),
            "sync-history" => {
                let _ = history_sync(None);
            }
            "open-codex" => {
                let _ = open_codex();
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            install_tray(app)?;
            show_main_window(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            save_profile,
            delete_profile,
            activate_profile,
            codex_status,
            clear_api_mode,
            history_status,
            history_sync,
            list_history_roots,
            save_history_root,
            delete_history_root,
            open_codex,
            diagnostics,
            list_backups,
            restore_backup
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{hydrate_profiles, StoredProfile};
    use remote_codex_core::profile::CodexProfile;
    use remote_codex_core::secrets::{InMemorySecretStore, SecretStore};

    fn profile(id: &str, name: &str) -> CodexProfile {
        CodexProfile {
            id: id.to_string(),
            name: name.to_string(),
            provider_name: name.to_string(),
            base_url: "https://jmrai.net/v1".to_string(),
            model: "gpt-5.5".to_string(),
            requires_openai_auth: true,
            keep_chatgpt_login: true,
            codex_home: None,
            created_at: "2026-05-17T00:00:00Z".to_string(),
            updated_at: "2026-05-17T00:00:00Z".to_string(),
        }
    }

    fn token_ready(profiles: &[StoredProfile], id: &str) -> bool {
        profiles
            .iter()
            .find(|profile| profile.profile.id == id)
            .map(|profile| profile.token_ready)
            .unwrap_or(false)
    }

    #[test]
    fn hydrate_profiles_marks_key_presence_per_profile() {
        let secrets = InMemorySecretStore::default();
        secrets.set_token("jmrai", "secret-token").unwrap();

        let profiles = hydrate_profiles(
            vec![
                profile("jmrai", "JMRAPI"),
                profile("openrouter", "OpenRouter"),
            ],
            &secrets,
        )
        .unwrap();

        assert!(token_ready(&profiles, "jmrai"));
        assert!(!token_ready(&profiles, "openrouter"));
    }

    #[test]
    fn hydrate_profiles_treats_blank_key_as_missing() {
        let secrets = InMemorySecretStore::default();
        secrets.set_token("jmrai", "   ").unwrap();

        let profiles = hydrate_profiles(vec![profile("jmrai", "JMRAPI")], &secrets).unwrap();

        assert!(!token_ready(&profiles, "jmrai"));
    }
}
