use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ClientSettings {
    pub server_url: String,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            server_url: "http://127.0.0.1:47821".into(),
        }
    }
}

fn settings_path(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app data dir")
        .join("TimeShards")
        .join("client-settings.json")
}

fn load_settings(app: &tauri::AppHandle) -> ClientSettings {
    let path = settings_path(app);
    if path.exists() {
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(s) = serde_json::from_str(&data) {
                return s;
            }
        }
    }
    ClientSettings::default()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![get_client_settings, save_client_settings])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn get_client_settings(app: tauri::AppHandle) -> ClientSettings {
    load_settings(&app)
}

#[tauri::command]
fn save_client_settings(app: tauri::AppHandle, settings: ClientSettings) -> Result<(), String> {
    let path = settings_path(&app);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}
