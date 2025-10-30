#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use tauri::Manager;

#[cfg(target_os = "macos")]
mod platform;

#[cfg(target_os = "macos")]
use platform::{
  add_extension_inner, check_full_disk_access_inner, list_file_associations_inner,
  open_full_disk_access_settings_inner, set_default_application_for_extension_inner,
};

#[cfg(not(target_os = "macos"))]
mod platform {
  use super::{FileAssociation, DEFAULT_EXTENSIONS};

  pub fn check_full_disk_access_inner() -> Result<bool, String> {
    Ok(true)
  }

  pub fn open_full_disk_access_settings_inner() -> Result<(), String> {
    Err("仅支持在 macOS 上打开系统设置".into())
  }

  pub fn list_file_associations_inner() -> Result<Vec<FileAssociation>, String> {
    Ok(
      DEFAULT_EXTENSIONS
        .iter()
        .map(|ext| FileAssociation {
          extension: ext.to_string(),
          application_name: "Unsupported platform".into(),
          application_path: String::new(),
        })
        .collect(),
    )
  }

  pub fn add_extension_inner(_extension: String) -> Result<Vec<FileAssociation>, String> {
    list_file_associations_inner()
  }

  pub fn set_default_application_for_extension_inner(
    _extension: String,
    _application_path: String,
  ) -> Result<(), String> {
    Err("仅支持在 macOS 上修改默认应用".into())
  }
}

// File extensions we care about by default. Keep in sync with the frontend list.
const DEFAULT_EXTENSIONS: &[&str] = &[
  // Documents
  "doc", "docx", "xls", "xlsx", "ppt", "pptx", "pdf", "txt", "md", "markdown",
  // Images
  "png", "jpg", "jpeg", "gif",
  // Media
  "mp3", "mp4", "mov", "avi",
  // Archives
  "zip", "rar", "7z", "tar", "gz",
  // Web
  "html", "htm", "css", "js", "ts", "jsx", "tsx",
  // Data / config
  "csv", "json", "xml", "yaml", "yml", "toml",
  // Code
  "py", "java", "cpp", "c", "h", "hpp",
  // Scripts
  "sh", "bash", "zsh", "fish",
  // DB / logs / misc
  "sql", "db", "sqlite", "log", "ini", "cfg", "conf",
  // Dev files
  "dockerfile", "gitignore", "env", "key", "pem", "crt",
];

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileAssociation {
  pub extension: String,
  pub application_name: String,
  pub application_path: String,
}

#[tauri::command]
fn check_full_disk_access() -> Result<bool, String> {
  check_full_disk_access_inner()
}

#[tauri::command]
fn open_full_disk_access_settings() -> Result<(), String> {
  open_full_disk_access_settings_inner()
}

#[tauri::command]
fn list_file_associations() -> Result<Vec<FileAssociation>, String> {
  list_file_associations_inner()
}

#[tauri::command]
fn add_extension(extension: String) -> Result<Vec<FileAssociation>, String> {
  add_extension_inner(extension)
}

#[tauri::command]
fn set_default_application_for_extension(
  extension: String,
  application_path: String,
) -> Result<(), String> {
  set_default_application_for_extension_inner(extension, application_path)
}

fn main() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![
      check_full_disk_access,
      open_full_disk_access_settings,
      list_file_associations,
      add_extension,
      set_default_application_for_extension
    ])
    .setup(|app| {
      #[cfg(target_os = "macos")]
      {
        if let Some(window) = app.get_webview_window("main") {
          let _ = window.set_focus();
        }
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
