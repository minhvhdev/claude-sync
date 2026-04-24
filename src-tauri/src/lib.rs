pub mod error;
mod git;
mod sync;
mod startup;

use error::AppError;
use git::GitEngine;
use sync::{SyncEngine, SyncState, SyncStatus};
use startup::StartupManager;

use std::sync::Mutex;
use tauri::{
    AppHandle, Manager, State,
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent},
    Emitter,
};
use log::{info, error, LevelFilter};
use std::time::SystemTime;



#[derive(serde::Serialize, serde::Deserialize, Default)]
struct AppConfig {
    github_token: Option<String>,
    repo_url: Option<String>,
    sync_credentials: Option<bool>,
}

impl AppConfig {
    fn get_path() -> std::path::PathBuf {
        dirs::home_dir()
            .map(|p| p.join(".claude").join("app_config.json"))
            .unwrap_or_else(|| std::path::PathBuf::from(".claude/app_config.json"))
    }

    fn load() -> Self {
        let path = Self::get_path();
        if let Ok(data) = std::fs::read_to_string(path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn save(&self) -> Result<(), AppError> {
        let path = Self::get_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let data = serde_json::to_string_pretty(self).map_err(|e| AppError::system(e.to_string()))?;
        std::fs::write(path, data).map_err(|e| AppError::system(e.to_string()))
    }
}

struct AppState {
    git_engine: Mutex<Option<GitEngine>>,
    sync_engine: Mutex<SyncEngine>,
    repo_url: Mutex<Option<String>>,
    token: Mutex<Option<String>>,
}

use simplelog::*;
use std::fs::OpenOptions;

fn init_logger() {
    let log_dir = dirs::home_dir()
        .map(|p| p.join(".claude").join("logs"))
        .unwrap_or_else(|| std::path::PathBuf::from(".claude/logs"));
    
    std::fs::create_dir_all(&log_dir).ok();
    let log_file = log_dir.join("claude_sync.log");

    CombinedLogger::init(
        vec![
            TermLogger::new(
                LevelFilter::Info,
                Config::default(),
                TerminalMode::Mixed,
                ColorChoice::Auto
            ),
            WriteLogger::new(
                LevelFilter::Info,
                Config::default(),
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_file)
                    .unwrap_or_else(|_| std::fs::File::create(&log_file).unwrap())
            ),
        ]
    ).unwrap_or_else(|e| eprintln!("Failed to initialize simplelog: {}", e));
}

#[tauri::command]
fn get_sync_state(state: State<AppState>) -> Result<SyncState, AppError> {
    let sync_engine = state.sync_engine.lock().map_err(|e| AppError::system(e.to_string()))?;
    Ok(sync_engine.get_state().clone())
}

#[tauri::command]
fn configure(repo_url: String, token: String, state: State<AppState>) -> Result<(), AppError> {
    info!("Configuring with repo URL: {}", repo_url);

    let mut sync_engine = state.sync_engine.lock().map_err(|e| AppError::system(e.to_string()))?;
    sync_engine.ensure_dirs_exist()?;

    let repo_path = sync_engine.get_repo_dir().to_string_lossy().to_string();

    let mut git_engine = GitEngine::new(repo_path, token.clone());
    git_engine.clone_or_open(&repo_url)?;

    // Save configuration to JSON
    let mut config = AppConfig::load();
    config.github_token = Some(token.clone());
    config.repo_url = Some(repo_url.clone());
    if let Err(e) = config.save() {
        error!("Failed to save config: {}", e);
    } else {
        info!("Configuration saved to app_config.json");
    }

    let mut git = state.git_engine.lock().map_err(|e| AppError::system(e.to_string()))?;
    *git = Some(git_engine);

    let mut url = state.repo_url.lock().map_err(|e| AppError::system(e.to_string()))?;
    *url = Some(repo_url);

    let mut tok = state.token.lock().map_err(|e| AppError::system(e.to_string()))?;
    *tok = Some(token);

    sync_engine.set_status(SyncStatus::Synced);
    sync_engine.set_last_sync(SystemTime::now());

    Ok(())
}

#[tauri::command]
fn get_saved_configuration(state: State<AppState>) -> Result<bool, AppError> {
    let config = AppConfig::load();
    
    let stored_token = match config.github_token {
        Some(t) => t,
        None => {
            error!("No token found in config");
            return Ok(false);
        }
    };
    
    let stored_repo = match config.repo_url {
        Some(r) => r,
        None => {
            error!("No repo URL found in config");
            return Ok(false);
        }
    };

    info!("Restoring saved configuration for repo: {}", stored_repo);
    let mut sync_engine = state.sync_engine.lock().map_err(|e| AppError::system(e.to_string()))?;
    sync_engine.ensure_dirs_exist()?;
    
    let repo_path = sync_engine.get_repo_dir().to_string_lossy().to_string();
    let mut git_engine = GitEngine::new(repo_path, stored_token.clone());
    
    if let Err(e) = git_engine.clone_or_open(&stored_repo) {
        error!("Failed to initialize git engine from saved config: {}", e);
        return Ok(false);
    }

    let mut git = state.git_engine.lock().map_err(|e| AppError::system(e.to_string()))?;
    *git = Some(git_engine);

    let mut url = state.repo_url.lock().map_err(|e| AppError::system(e.to_string()))?;
    *url = Some(stored_repo);

    let mut tok = state.token.lock().map_err(|e| AppError::system(e.to_string()))?;
    *tok = Some(stored_token);

    if let Some(enabled) = config.sync_credentials {
        sync_engine.set_sync_credentials(enabled);
    }

    sync_engine.set_status(SyncStatus::Synced);
    
    Ok(true)
}

#[tauri::command]
fn test_connection(_repo_url: String, token: String) -> Result<(), AppError> {
    info!("Testing connection to GitHub API");

    let client = reqwest::blocking::Client::builder()
        .user_agent("ClaudeSync/1.0")
        .build()
        .map_err(|e| AppError::network(e.to_string()))?;

    let res = client.get("https://api.github.com/user")
        .bearer_auth(token)
        .send()
        .map_err(|e| AppError::network(e.to_string()))?;

    if res.status().is_success() {
        info!("Connection test successful");
        Ok(())
    } else {
        Err(AppError::auth(format!("GitHub API Error: {}", res.status())))
    }
}

#[tauri::command]
fn create_github_repo(token: String) -> Result<String, AppError> {
    info!("Attempting to create claude-settings repo on GitHub");

    let client = reqwest::blocking::Client::builder()
        .user_agent("ClaudeSync/1.0")
        .build()
        .map_err(|e| AppError::network(e.to_string()))?;

    let body = serde_json::json!({
        "name": "claude-settings",
        "description": "Claude Sync Settings Repository",
        "private": true
    });

    let res = client.post("https://api.github.com/user/repos")
        .bearer_auth(&token)
        .json(&body)
        .send()
        .map_err(|e| AppError::network(e.to_string()))?;

    if res.status().is_success() || res.status() == 422 { 
        // 422 usually means the repo already exists, but we are asked to create it if it doesn't.
        // Wait, if 422, we should fetch the repo instead of returning error.
        if res.status() == 422 {
            let get_res = client.get("https://api.github.com/user/repos")
                .bearer_auth(&token)
                .send()
                .map_err(|e| AppError::network(e.to_string()))?;
            
            if get_res.status().is_success() {
                let repos: Vec<serde_json::Value> = get_res.json().unwrap_or(vec![]);
                if let Some(existing) = repos.into_iter().find(|r| r["name"] == "claude-settings") {
                     return Ok(existing["clone_url"].as_str().unwrap_or("").to_string());
                }
            }
            return Err(AppError::network("Repo already exists but could not retrieve it.".to_string()));
        }

        let data: serde_json::Value = res.json().map_err(|e| AppError::network(e.to_string()))?;
        let clone_url = data["clone_url"].as_str().unwrap_or("").to_string();
        Ok(clone_url)
    } else {
        Err(AppError::network(format!("Failed to create repo: {}", res.status())))
    }
}

#[tauri::command]
fn pull_sync(state: State<AppState>) -> Result<(), AppError> {
    info!("Manual Pull triggered");

    let git_engine = state.git_engine.lock().map_err(|e| AppError::system(e.to_string()))?;
    let mut sync_engine = state.sync_engine.lock().map_err(|e| AppError::system(e.to_string()))?;

    sync_engine.set_status(SyncStatus::Syncing);

    if let Some(ref git) = *git_engine {
        if let Err(e) = git.pull() {
            sync_engine.set_status(SyncStatus::Error);
            return Err(e);
        }
    } else {
        sync_engine.set_status(SyncStatus::Error);
        return Err(AppError::system("Configuration missing"));
    }

    if let Err(e) = sync_engine.copy_to_local(&[]) {
        sync_engine.set_status(SyncStatus::Error);
        return Err(e);
    }

    sync_engine.set_status(SyncStatus::Synced);
    sync_engine.set_last_sync(SystemTime::now());
    Ok(())
}

#[tauri::command]
fn push_sync(state: State<AppState>) -> Result<(), AppError> {
    info!("Manual Push triggered");

    let git_engine = state.git_engine.lock().map_err(|e| AppError::system(e.to_string()))?;
    let mut sync_engine = state.sync_engine.lock().map_err(|e| AppError::system(e.to_string()))?;

    sync_engine.set_status(SyncStatus::Syncing);

    if let Err(e) = sync_engine.copy_from_local(&[]) {
        sync_engine.set_status(SyncStatus::Error);
        return Err(e);
    }

    if let Some(ref git) = *git_engine {
        if let Err(e) = git.add_all_and_commit("manual push") {
            sync_engine.set_status(SyncStatus::Error);
            return Err(e);
        }
        if let Err(e) = git.push("manual push") {
            sync_engine.set_status(SyncStatus::Error);
            return Err(e);
        }
    } else {
        sync_engine.set_status(SyncStatus::Error);
        return Err(AppError::system("Configuration missing"));
    }

    sync_engine.set_status(SyncStatus::Synced);
    sync_engine.set_last_sync(SystemTime::now());

    Ok(())
}

#[tauri::command]
fn get_autostart_enabled(app: AppHandle) -> bool {
    StartupManager::is_autostart_enabled(&app)
}

#[tauri::command]
fn set_autostart(enabled: bool, app: AppHandle) -> Result<(), AppError> {
    info!("Set autostart: {}", enabled);
    if enabled {
        StartupManager::enable_autostart(&app).map_err(|e| AppError::system(e))
    } else {
        StartupManager::disable_autostart(&app).map_err(|e| AppError::system(e))
    }
}

#[tauri::command]
fn set_sync_credentials(enabled: bool, state: State<AppState>) -> Result<(), AppError> {
    info!("Set sync credentials: {}", enabled);
    let mut sync_engine = state.sync_engine.lock().map_err(|e| AppError::system(e.to_string()))?;
    sync_engine.set_sync_credentials(enabled);

    let mut config = AppConfig::load();
    config.sync_credentials = Some(enabled);
    if let Err(e) = config.save() {
        error!("Failed to save sync_credentials to config: {}", e);
    }

    Ok(())
}

#[tauri::command]
fn get_repo_url(state: State<AppState>) -> Option<String> {
    state.repo_url.lock().ok().and_then(|r| r.clone())
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let push_menu = MenuItem::with_id(app, "push_sync", "Push to GitHub", true, None::<&str>)?;
    let pull_menu = MenuItem::with_id(app, "pull_sync", "Pull from GitHub", true, None::<&str>)?;
    let open_settings = MenuItem::with_id(app, "open_settings", "Open Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&push_menu, &pull_menu, &open_settings, &quit])?;

    let mut tray_builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Claude Sync");

    if let Some(icon) = app.default_window_icon() {
        tray_builder = tray_builder.icon(icon.clone());
    }

    let _tray = tray_builder
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "push_sync" => {
                    info!("Tray: Push to GitHub clicked");
                    let _ = app.emit("tray-push-sync", ());
                }
                "pull_sync" => {
                    info!("Tray: Pull from GitHub clicked");
                    let _ = app.emit("tray-pull-sync", ());
                }
                "open_settings" => {
                    info!("Tray: Open Settings clicked");
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    info!("Tray: Quit clicked");
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                info!("Tray icon clicked");
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logger();
    info!("Claude Sync starting...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--flag"]),
        ))
        .manage(AppState {
            git_engine: Mutex::new(None),
            sync_engine: Mutex::new(SyncEngine::new()),
            repo_url: Mutex::new(None),
            token: Mutex::new(None),
        })
        .setup(|app| {
            info!("Setting up Claude Sync application");

            setup_tray(app)?;

            info!("Claude Sync setup complete");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Prevent the app from exiting when window is closed
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_sync_state,
            configure,
            get_saved_configuration,
            test_connection,
            create_github_repo,
            pull_sync,
            push_sync,
            get_autostart_enabled,
            set_autostart,
            set_sync_credentials,
            get_repo_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
