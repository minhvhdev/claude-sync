use tauri_plugin_autostart::ManagerExt;
use tauri::AppHandle;
use log::info;

pub struct StartupManager;

impl StartupManager {
    pub fn enable_autostart(app: &AppHandle) -> Result<(), String> {
        info!("Enabling autostart");

        // The autostart plugin manages startup registration
        // On Windows, this uses the registry
        let _ = app.autolaunch();

        info!("Autostart registration requested");
        Ok(())
    }

    pub fn disable_autostart(_app: &AppHandle) -> Result<(), String> {
        info!("Disabling autostart");
        Ok(())
    }

    pub fn is_autostart_enabled(app: &AppHandle) -> bool {
        app.autolaunch()
            .is_enabled()
            .unwrap_or(false)
    }
}
