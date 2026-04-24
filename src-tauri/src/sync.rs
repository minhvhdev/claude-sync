use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use log::{info, warn};
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub status: SyncStatus,
    pub last_sync: Option<u64>,
    pub watcher_enabled: bool,
    pub pending_changes: bool,
    pub sync_credentials: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncStatus {
    Synced,
    Syncing,
    Error,
    NotConfigured,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            status: SyncStatus::NotConfigured,
            last_sync: None,
            watcher_enabled: true,
            pending_changes: false,
            sync_credentials: false,
        }
    }
}

pub struct SyncEngine {
    state: SyncState,
    claude_dir: PathBuf,
    repo_dir: PathBuf,
}

impl SyncEngine {
    pub fn new() -> Self {
        let claude_dir = dirs::home_dir()
            .map(|p| p.join(".claude"))
            .unwrap_or_else(|| PathBuf::from(".claude"));

        let repo_dir = claude_dir.join("repo");

        Self {
            state: SyncState::default(),
            claude_dir,
            repo_dir,
        }
    }

    pub fn get_state(&self) -> &SyncState {
        &self.state
    }

    pub fn set_status(&mut self, status: SyncStatus) {
        self.state.status = status;
    }

    pub fn set_sync_credentials(&mut self, enabled: bool) {
        self.state.sync_credentials = enabled;
    }

    pub fn set_last_sync(&mut self, time: SystemTime) {
        self.state.last_sync = time
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs());
    }

    pub fn _set_pending_changes(&mut self, pending: bool) {
        self.state.pending_changes = pending;
    }

    pub fn get_repo_dir(&self) -> &PathBuf {
        &self.repo_dir
    }

    pub fn ensure_dirs_exist(&self) -> Result<(), AppError> {
        fs::create_dir_all(&self.claude_dir)
            .map_err(|e| AppError::io(format!("Failed to create .claude directory: {}", e)))?;
        fs::create_dir_all(&self.repo_dir)
            .map_err(|e| AppError::io(format!("Failed to create repo directory: {}", e)))?;
        Ok(())
    }

    pub fn copy_to_local(&self, _files: &[&str]) -> Result<(), AppError> {
        info!("Copying files from repo to local");
        let sync_items = self.get_sync_items();

        for item in &sync_items {
            let src = self.repo_dir.join(item);
            let dst = self.claude_dir.join(item);

            if !src.exists() {
                continue;
            }

            if src.is_dir() {
                copy_dir_recursive(&src, &dst)?;
            } else {
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent).map_err(|e| AppError::io(e.to_string()))?;
                }
                fs::copy(&src, &dst).map_err(|e| AppError::io(e.to_string()))?;
            }
            info!("Copied {} to {:?}", item, dst);
        }

        // Handle .claude.json which is in USERPROFILE, not in .claude/
        let src = self.repo_dir.join(".claude.json");
        let dst = dirs::home_dir()
            .ok_or_else(|| AppError::system("Could not find home directory"))?
            .join(".claude.json");

        if src.exists() {
            fs::copy(&src, &dst).map_err(|e| AppError::io(e.to_string()))?;
            info!("Copied .claude.json to {:?}", dst);
        }

        Ok(())
    }

    pub fn copy_from_local(&self, _files: &[&str]) -> Result<(), AppError> {
        info!("Copying files from local to repo");
        let sync_items = self.get_sync_items();

        for item in &sync_items {
            let src = self.claude_dir.join(item);
            let dst = self.repo_dir.join(item);

            if !src.exists() {
                warn!("Source file not found: {:?}", src);
                continue;
            }

            if src.is_dir() {
                copy_dir_recursive(&src, &dst)?;
            } else {
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent).map_err(|e| AppError::io(e.to_string()))?;
                }
                fs::copy(&src, &dst).map_err(|e| AppError::io(e.to_string()))?;
            }
            info!("Copied {} to {:?}", item, dst);
        }

        // Handle .claude.json which is in USERPROFILE, not in .claude/
        let src = dirs::home_dir()
            .ok_or_else(|| AppError::system("Could not find home directory"))?
            .join(".claude.json");
        let dst = self.repo_dir.join(".claude.json");

        if src.exists() {
            fs::copy(&src, &dst).map_err(|e| AppError::io(e.to_string()))?;
            info!("Copied .claude.json to {:?}", dst);
        }

        Ok(())
    }

    fn get_sync_items(&self) -> Vec<String> {
        let mut items = vec![
            "settings.json".to_string(),
            "CLAUDE.md".to_string(),
            "commands".to_string(),
            "agents".to_string(),
            "skills".to_string(),
            "plugins".to_string(),
        ];
        if self.state.sync_credentials {
            items.push(".credentials.json".to_string());
        }
        items
    }

    pub fn _save_pending_flag(&self) -> Result<(), String> {
        let flag_path = self.claude_dir.join("pending_sync");
        fs::write(&flag_path, "1").map_err(|e| e.to_string())?;
        info!("Pending sync flag saved");
        Ok(())
    }

    pub fn _clear_pending_flag(&self) -> Result<(), String> {
        let flag_path = self.claude_dir.join("pending_sync");
        if flag_path.exists() {
            fs::remove_file(&flag_path).map_err(|e| e.to_string())?;
            info!("Pending sync flag cleared");
        }
        Ok(())
    }

    pub fn _has_pending_flag(&self) -> bool {
        self.claude_dir.join("pending_sync").exists()
    }
}

fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> Result<(), AppError> {
    if !dst.exists() {
        fs::create_dir_all(dst).map_err(|e| AppError::io(e.to_string()))?;
    }

    for entry in fs::read_dir(src).map_err(|e| AppError::io(e.to_string()))? {
        let entry = entry.map_err(|e| AppError::io(e.to_string()))?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest)?;
        } else {
            fs::copy(&path, &dest).map_err(|e| AppError::io(e.to_string()))?;
        }
    }

    Ok(())
}
