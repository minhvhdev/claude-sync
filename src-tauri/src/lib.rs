mod git;
mod watcher;
mod sync;
mod startup;

use git::GitEngine;
use watcher::FileWatcher;
use sync::{SyncEngine, SyncState, SyncStatus};
use startup::StartupManager;

use std::sync::Mutex;
use tauri::{
    AppHandle, Manager, State,
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent},
    Emitter,
};
use log::{info, LevelFilter};
use env_logger::Builder;
use std::io::Write;
use std::time::SystemTime;

struct AppState {
    git_engine: Mutex<Option<GitEngine>>,
    sync_engine: Mutex<SyncEngine>,
    watcher: Mutex<FileWatcher>,
    repo_url: Mutex<Option<String>>,
    token: Mutex<Option<String>>,
    watcher_enabled: Mutex<bool>,
}

fn init_logger() {
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] {} - {}",
                chrono_lite(),
                record.level(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .init();
}

fn chrono_lite() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}", now)
}

#[tauri::command]
fn get_sync_state(state: State<AppState>) -> Result<SyncState, String> {
    let sync_engine = state.sync_engine.lock().map_err(|e| e.to_string())?;
    Ok(sync_engine.get_state().clone())
}

#[tauri::command]
fn configure(repo_url: String, token: String, state: State<AppState>) -> Result<(), String> {
    info!("Configuring with repo URL: {}", repo_url);

    let mut sync_engine = state.sync_engine.lock().map_err(|e| e.to_string())?;
    sync_engine.ensure_dirs_exist()?;

    let repo_path = sync_engine.get_repo_dir().to_string_lossy().to_string();

    let mut git_engine = GitEngine::new(repo_path, token.clone());
    git_engine.clone_or_open(&repo_url)?;

    let mut git = state.git_engine.lock().map_err(|e| e.to_string())?;
    *git = Some(git_engine);

    let mut url = state.repo_url.lock().map_err(|e| e.to_string())?;
    *url = Some(repo_url);

    let mut tok = state.token.lock().map_err(|e| e.to_string())?;
    *tok = Some(token);

    sync_engine.set_status(SyncStatus::Synced);
    sync_engine.set_last_sync(SystemTime::now());

    Ok(())
}

#[tauri::command]
fn test_connection(repo_url: String, token: String) -> Result<(), String> {
    info!("Testing connection to: {}", repo_url);

    let temp_path = std::env::temp_dir().join("claude-sync-test-repo");
    let _ = std::fs::remove_dir_all(&temp_path);

    let mut engine = GitEngine::new(temp_path.to_string_lossy().to_string(), token);
    engine.clone_or_open(&repo_url)?;

    let _ = std::fs::remove_dir_all(&temp_path);
    info!("Connection test successful");
    Ok(())
}

#[tauri::command]
fn sync_now(state: State<AppState>) -> Result<(), String> {
    info!("Manual sync triggered");

    let git_engine = state.git_engine.lock().map_err(|e| e.to_string())?;
    let mut sync_engine = state.sync_engine.lock().map_err(|e| e.to_string())?;

    sync_engine.set_status(SyncStatus::Syncing);

    if let Some(ref git) = *git_engine {
        git.pull()?;
    }

    sync_engine.copy_to_local(&[])?;

    drop(git_engine);

    let git_engine = state.git_engine.lock().map_err(|e| e.to_string())?;
    if let Some(ref git) = *git_engine {
        git.add_all_and_commit("manual sync")?;
        git.push("manual sync")?;
    }

    let mut sync_engine = state.sync_engine.lock().map_err(|e| e.to_string())?;
    sync_engine.set_status(SyncStatus::Synced);
    sync_engine.set_last_sync(SystemTime::now());

    Ok(())
}

#[tauri::command]
fn toggle_watcher(enabled: bool, state: State<AppState>, app: AppHandle) -> Result<(), String> {
    info!("Toggle watcher: {}", enabled);

    let mut watcher_enabled = state.watcher_enabled.lock().map_err(|e| e.to_string())?;
    *watcher_enabled = enabled;

    let mut watcher = state.watcher.lock().map_err(|e| e.to_string())?;
    let sync_engine = state.sync_engine.lock().map_err(|e| e.to_string())?;

    if enabled {
        let path = sync_engine.get_claude_dir().clone();
        let app_handle = app.clone();
        watcher.start(path, move || {
            info!("File change detected, triggering auto-sync");
            let _ = app_handle.emit("trigger-sync", ());
        })?;
    } else {
        watcher.stop();
    }

    let mut sync_engine = state.sync_engine.lock().map_err(|e| e.to_string())?;
    sync_engine.set_watcher_enabled(enabled);

    Ok(())
}

#[tauri::command]
fn get_autostart_enabled(app: AppHandle) -> bool {
    StartupManager::is_autostart_enabled(&app)
}

#[tauri::command]
fn set_autostart(enabled: bool, app: AppHandle) -> Result<(), String> {
    info!("Set autostart: {}", enabled);
    if enabled {
        StartupManager::enable_autostart(&app)
    } else {
        StartupManager::disable_autostart(&app)
    }
}

#[tauri::command]
fn get_repo_url(state: State<AppState>) -> Option<String> {
    state.repo_url.lock().ok().and_then(|r| r.clone())
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let sync_now = MenuItem::with_id(app, "sync_now", "Sync Now", true, None::<&str>)?;
    let open_settings = MenuItem::with_id(app, "open_settings", "Open Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&sync_now, &open_settings, &quit])?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Claude Sync")
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "sync_now" => {
                    info!("Tray: Sync Now clicked");
                    let _ = app.emit("tray-sync-now", ());
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
            watcher: Mutex::new(FileWatcher::new()),
            repo_url: Mutex::new(None),
            token: Mutex::new(None),
            watcher_enabled: Mutex::new(true),
        })
        .setup(|app| {
            info!("Setting up Claude Sync application");

            setup_tray(app)?;

            let _app_handle = app.handle().clone();

            // Auto-sync on startup after a short delay
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(2));
                info!("Startup sync triggered");
                let _ = handle.emit("trigger-sync", ());
            });

            info!("Claude Sync setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_sync_state,
            configure,
            test_connection,
            sync_now,
            toggle_watcher,
            get_autostart_enabled,
            set_autostart,
            get_repo_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
