use crate::{FileAssociation, DEFAULT_EXTENSIONS};
use plist::{Dictionary, Value};
use std::collections::BTreeSet;
use std::env;
use std::ffi::{c_char, c_void, CString};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use url::Url;

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFAllocatorRef = *const c_void;

const CFSTRING_ENCODING_UTF8: u32 = 0x0800_0100;

const EXTENSION_TO_CONTENT_TYPE: &[(&str, &str)] = &[
  // Office
  ("doc", "com.microsoft.word.doc"),
  ("docx", "org.openxmlformats.wordprocessingml.document"),
  ("xls", "com.microsoft.excel.xls"),
  ("xlsx", "org.openxmlformats.spreadsheetml.sheet"),
  ("ppt", "com.microsoft.powerpoint.ppt"),
  ("pptx", "org.openxmlformats.presentationml.presentation"),
  ("txt", "public.plain-text"),
  ("pdf", "com.adobe.pdf"),
  ("png", "public.png"),
  ("jpg", "public.jpeg"),
  ("jpeg", "public.jpeg"),
  ("gif", "public.gif"),
  ("csv", "public.comma-separated-values-text"),
  ("mp3", "public.mp3"),
  ("mp4", "public.mpeg-4"),
  ("mov", "com.apple.quicktime-movie"),
  ("avi", "public.avi"),
  ("zip", "public.zip-archive"),
  ("rar", "public.rar-archive"),
  ("7z", "public.7z-archive"),
  ("tar", "public.tar-archive"),
  ("gz", "public.gzip-archive"),
  ("json", "public.json"),
  ("xml", "public.xml"),
  ("html", "public.html"),
  ("htm", "public.html"),
  ("css", "public.css"),
  ("js", "public.javascript"),
  ("ts", "public.typescript"),
  ("jsx", "public.jsx"),
  ("tsx", "public.tsx"),
  ("md", "net.daringfireball.markdown"),
  ("markdown", "net.daringfireball.markdown"),
  ("py", "public.python-script"),
  ("java", "com.sun.java-source"),
  ("cpp", "public.c-plus-plus-source"),
  ("c", "public.c-source"),
  ("h", "public.c-header"),
  ("hpp", "public.c-plus-plus-header"),
  ("sh", "public.shell-script"),
  ("bash", "public.shell-script"),
  ("zsh", "public.shell-script"),
  ("fish", "public.shell-script"),
  ("sql", "public.sql-source"),
  ("db", "public.database"),
  ("sqlite", "public.sqlite3-database"),
  ("log", "public.log"),
  ("ini", "public.ini"),
  ("cfg", "public.configuration"),
  ("conf", "public.configuration"),
  ("yaml", "public.yaml"),
  ("yml", "public.yaml"),
  ("toml", "public.toml"),
  ("env", "public.environment"),
  ("key", "public.private-key"),
  ("pem", "public.pem"),
  ("crt", "public.certificate"),
];

const CONFIG_DIR_NAME: &str = "Default Application Manager";
const EXTENSIONS_FILE_NAME: &str = "extensions.json";

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
  static kCFAllocatorDefault: CFAllocatorRef;
  fn CFStringCreateWithCString(
    alloc: CFAllocatorRef,
    c_str: *const c_char,
    encoding: u32,
  ) -> CFStringRef;
  fn CFStringGetCString(
    the_string: CFStringRef,
    buffer: *mut c_char,
    buffer_size: isize,
    encoding: u32,
  ) -> u8;
  fn CFRelease(cf: CFTypeRef);
}

#[derive(Debug, Error)]
enum PlatformError {
  #[error("无法获取用户目录: {0}")]
  HomeUnavailable(#[from] env::VarError),
  #[error("选择的路径无效: {0}")]
  InvalidSelection(String),
  #[error("配置读写失败: {0}")]
  Config(String),
  #[error("IO 错误: {0}")]
  Io(#[from] std::io::Error),
  #[error("Plist 解析失败: {0}")]
  Plist(#[from] plist::Error),
  #[error("缺少 LSHandlers 配置")]
  MissingHandlers,
  #[error("命令执行失败: {0}")]
  Command(String),
  #[error("应用信息缺少字段: {0}")]
  MissingInfo(String),
}

pub fn check_full_disk_access_inner() -> Result<bool, String> {
  use std::fs::File;

  // Probe a set of known protected files. If any can be opened, FDA is granted.
  let mut probe_paths = vec![PathBuf::from(
    "/Library/Application Support/com.apple.TCC/TCC.db",
  )];

  if let Ok(home) = env::var("HOME") {
    probe_paths.push(
      PathBuf::from(&home)
        .join("Library/Preferences/com.apple.LaunchServices/com.apple.launchservices.secure.plist"),
    );
    probe_paths.push(PathBuf::from(&home).join("Library/Safari/History.db"));
    probe_paths.push(PathBuf::from(&home).join("Library/Messages/chat.db"));
  }

  let mut _saw_permission_denied = false;
  for path in probe_paths {
    match File::open(&path) {
      Ok(_) => return Ok(true),
      Err(err) if err.kind() == ErrorKind::PermissionDenied => {
        _saw_permission_denied = true
      }
      Err(err) if err.kind() == ErrorKind::NotFound => continue,
      Err(err) => return Err(format!("检测权限失败: {err}")),
    }
  }

  // If any access was denied, or no probes existed, be conservative: not granted.
  Ok(false)
}

pub fn open_full_disk_access_settings_inner() -> Result<(), String> {
  Command::new("open")
    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
    .status()
    .map_err(|err| err.to_string())
    .and_then(|status| {
      if status.success() {
        Ok(())
      } else {
        Err(format!("打开系统设置失败，退出状态: {status}"))
      }
    })
}

pub fn list_file_associations_inner() -> Result<Vec<FileAssociation>, String> {
  match list_file_associations_impl() {
    Ok(list) => Ok(list),
    Err(err) => Err(err.to_string()),
  }
}

pub fn add_extension_inner(extension: String) -> Result<Vec<FileAssociation>, String> {
  match add_extension_impl(extension) {
    Ok(list) => Ok(list),
    Err(err) => Err(err.to_string()),
  }
}

pub fn set_default_application_for_extension_inner(
  extension: String,
  application_path: String,
) -> Result<(), String> {
  match set_default_application_impl(extension, application_path) {
    Ok(()) => Ok(()),
    Err(err) => Err(err.to_string()),
  }
}

fn launch_services_plist_path() -> Result<PathBuf, PlatformError> {
  let home = env::var("HOME")?;
  Ok(PathBuf::from(home)
    .join("Library/Preferences/com.apple.LaunchServices/com.apple.launchservices.secure.plist"))
}

fn extensions_config_path() -> Result<PathBuf, PlatformError> {
  let home = env::var("HOME")?;
  Ok(
    PathBuf::from(&home)
      .join("Library")
      .join("Application Support")
      .join(CONFIG_DIR_NAME)
      .join(EXTENSIONS_FILE_NAME),
  )
}

fn load_extension_list() -> Result<Vec<String>, PlatformError> {
  let mut set: BTreeSet<String> = DEFAULT_EXTENSIONS
    .iter()
    .map(|ext| ensure_extension_normalized(ext))
    .collect();

  let path = extensions_config_path()?;
  if path.exists() {
    let text = fs::read_to_string(&path)?;
    let stored: Vec<String> =
      serde_json::from_str(&text).map_err(|err| PlatformError::Config(err.to_string()))?;
    for item in stored {
      let normalized = ensure_extension_normalized(&item);
      if !normalized.is_empty() {
        set.insert(normalized);
      }
    }
  }

  Ok(set.into_iter().collect())
}

fn save_extension_list(extensions: &[String]) -> Result<(), PlatformError> {
  let path = extensions_config_path()?;
  if let Some(dir) = path.parent() {
    fs::create_dir_all(dir)?;
  }

  let payload =
    serde_json::to_string_pretty(extensions).map_err(|err| PlatformError::Config(err.to_string()))?;
  fs::write(&path, payload)?;
  Ok(())
}

fn register_extension_if_needed(extension: &str) -> Result<(), PlatformError> {
  let mut set: BTreeSet<String> = load_extension_list()?.into_iter().collect();
  if set.insert(extension.to_string()) {
    let list: Vec<String> = set.into_iter().collect();
    save_extension_list(&list)?;
  }
  Ok(())
}

fn load_launch_services_value() -> Result<Value, PlatformError> {
  let path = launch_services_plist_path()?;
  let mut value = if path.exists() {
    Value::from_file(&path)?
  } else {
    Value::Dictionary(Dictionary::new())
  };

  if let Some(dict) = value.as_dictionary_mut() {
    if !dict.contains_key("LSHandlers") {
      dict.insert("LSHandlers".to_string(), Value::Array(Vec::new()));
    }
  }

  Ok(value)
}

fn handlers_from_value(value: &Value) -> Result<&Vec<Value>, PlatformError> {
  value
    .as_dictionary()
    .and_then(|dict| dict.get("LSHandlers"))
    .and_then(Value::as_array)
    .ok_or(PlatformError::MissingHandlers)
}

fn handlers_from_value_mut(value: &mut Value) -> Result<&mut Vec<Value>, PlatformError> {
  let dict = value
    .as_dictionary_mut()
    .ok_or(PlatformError::MissingHandlers)?;

  if !dict.contains_key("LSHandlers") {
    dict.insert("LSHandlers".into(), Value::Array(Vec::new()));
  }

  dict
    .get_mut("LSHandlers")
    .and_then(Value::as_array_mut)
    .ok_or(PlatformError::MissingHandlers)
}

fn find_bundle_id_for_extension(handlers: &[Value], extension: &str) -> Option<String> {
  let normalized = extension.to_lowercase();
  let content_type = extension_to_content_type(&normalized).map(str::to_string);

  handlers.iter().find_map(|item| {
    let dict = item.as_dictionary()?;
    let tag = dict
      .get("LSHandlerContentTag")
      .and_then(Value::as_string)
      .map(str::to_lowercase);

    let tag_class = dict
      .get("LSHandlerContentTagClass")
      .and_then(Value::as_string);

    let matches_extension =
      tag.as_deref() == Some(normalized.as_str()) && tag_class == Some("public.filename-extension");
    let matches_content_type = content_type.as_ref().and_then(|expected| {
      dict
        .get("LSHandlerContentType")
        .and_then(Value::as_string)
        .filter(|value| value == expected)
    }).is_some();

    if matches_extension || matches_content_type {
      dict
        .get("LSHandlerRoleAll")
        .and_then(Value::as_string)
        .map(|s| s.to_string())
        .or_else(|| {
          dict
            .get("LSHandlerRoleViewer")
            .and_then(Value::as_string)
            .map(|s| s.to_string())
        })
    } else {
      None
    }
  })
}

fn bundle_path_from_id(bundle_id: &str) -> Result<PathBuf, PlatformError> {
  // Avoid AppleScript automation prompts; use Spotlight index via mdfind
  // Query Spotlight for exact bundle identifier
  let query = format!("kMDItemCFBundleIdentifier == '{}'", bundle_id);
  let output = Command::new("mdfind").arg(query).output().map_err(PlatformError::Io)?;
  if output.status.success() {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let candidates: Vec<PathBuf> = stdout
      .lines()
      .filter(|line| line.trim().ends_with(".app"))
      .map(|line| PathBuf::from(line.trim()))
      .collect();

    // Verify candidates’ Info.plist identifier and prefer common application locations
    let preferred_prefixes = vec![
      PathBuf::from("/Applications"),
      PathBuf::from("/System/Applications"),
      PathBuf::from("/System/Applications/Utilities"),
      PathBuf::from(format!("{}/Applications", env::var("HOME").unwrap_or_default())),
    ];

    // First, try to match exact bundle id by reading Info.plist
    for p in &candidates {
      let info_path = p.join("Contents").join("Info.plist");
      if let Ok(value) = Value::from_file(&info_path) {
        if let Some(dict) = value.as_dictionary() {
          let id = dict.get("CFBundleIdentifier").and_then(Value::as_string);
          if id.map(|i| i.eq_ignore_ascii_case(bundle_id)).unwrap_or(false) {
            return Ok(p.clone());
          }
        }
      }
    }

    // Next, prefer by location
    if let Some(best) = candidates
      .iter()
      .find(|p| preferred_prefixes.iter().any(|pref| p.starts_with(pref)))
    {
      return Ok(best.clone());
    }

    // Finally, take the first result
    if let Some(first) = candidates.into_iter().next() {
      return Ok(first);
    }
  }

  // Fallback: scan common application folders and match Info.plist CFBundleIdentifier
  if let Some(found) = find_app_in_common_locations(bundle_id) {
    return Ok(found);
  }

  Err(PlatformError::Command("未找到应用路径".into()))
}

fn find_app_in_common_locations(bundle_id: &str) -> Option<PathBuf> {
  let mut roots = vec![
    PathBuf::from("/Applications"),
    PathBuf::from("/System/Applications"),
    PathBuf::from("/System/Applications/Utilities"),
  ];
  if let Ok(home) = env::var("HOME") {
    roots.push(PathBuf::from(home).join("Applications"));
  }

  for root in roots {
    let mut apps = Vec::new();
    collect_apps(&root, 2, &mut apps);
    // First, match by CFBundleIdentifier
    for path in &apps {
      let info_path = path.join("Contents").join("Info.plist");
      if let Ok(value) = Value::from_file(&info_path) {
        if let Some(dict) = value.as_dictionary() {
          let id = dict.get("CFBundleIdentifier").and_then(Value::as_string);
          if let Some(id) = id {
            let a = id.to_ascii_lowercase();
            let b = bundle_id.to_ascii_lowercase();
            if a == b || a.ends_with(&b) || b.ends_with(&a) {
              return Some(path.clone());
            }
          }
        }
      }
    }

    // Next, match by app folder name or CFBundleName hint
    let hint = bundle_id.rsplit('.').next().unwrap_or(bundle_id).to_ascii_lowercase();
    for path in apps {
      let stem = path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase());
      if stem.as_deref().map(|s| s.contains(&hint)).unwrap_or(false) {
        return Some(path);
      }
      let info_path = path.join("Contents").join("Info.plist");
      if let Ok(value) = Value::from_file(&info_path) {
        if let Some(dict) = value.as_dictionary() {
          let name = dict.get("CFBundleName").and_then(Value::as_string);
          if let Some(name) = name {
            if name.to_ascii_lowercase().contains(&hint) {
              return Some(path);
            }
          }
        }
      }
    }
  }
  None
}

fn collect_apps(root: &Path, depth: usize, acc: &mut Vec<PathBuf>) {
  if depth == 0 {
    return;
  }
  if let Ok(read_dir) = fs::read_dir(root) {
    for entry in read_dir.flatten() {
      let path = entry.path();
      if path.extension().map(|e| e.eq_ignore_ascii_case("app")).unwrap_or(false) {
        acc.push(path);
      } else if path.is_dir() {
        collect_apps(&path, depth - 1, acc);
      }
    }
  }
}

fn read_app_display_name(info_dict: &Dictionary, fallback: &Path) -> String {
  // 优先使用 Info.plist 中的显示名称，其次使用 CFBundleName，最后退回到包文件夹名
  if let Some(name) = info_dict
    .get("CFBundleDisplayName")
    .and_then(Value::as_string)
    .map(|s| s.to_string())
  {
    return name;
  }

  if let Some(name) = info_dict
    .get("CFBundleName")
    .and_then(Value::as_string)
    .map(|s| s.to_string())
  {
    return name;
  }

  fallback
    .file_stem()
    .and_then(|stem| stem.to_str())
    .unwrap_or("未知应用")
    .to_string()
}

fn application_name_from_path(app_path: &Path) -> Result<String, PlatformError> {
  // Prefer Info.plist values; fallback to Spotlight display name; finally use folder name
  let info_path = app_path.join("Contents").join("Info.plist");
  match Value::from_file(&info_path) {
    Ok(info_value) => {
      if let Some(dict) = info_value.as_dictionary() {
        return Ok(read_app_display_name(dict, app_path));
      }
    }
    Err(_) => {}
  }

  // Fallback via mdls metadata
  if let Some(name) = mdls_display_name(app_path) {
    return Ok(name);
  }

  Ok(
    app_path
      .file_stem()
      .and_then(|s| s.to_str())
      .unwrap_or("未知应用")
      .to_string(),
  )
}

fn mdls_display_name(app_path: &Path) -> Option<String> {
  let output = Command::new("mdls")
    .arg("-name")
    .arg("kMDItemDisplayName")
    .arg("-raw")
    .arg(app_path)
    .output()
    .ok()?;

  if !output.status.success() {
    return None;
  }
  let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
  if text.is_empty() || text == "(null)" {
    None
  } else {
    Some(text)
  }
}

fn bundle_id_from_path(app_path: &Path) -> Result<String, PlatformError> {
  let info_path = app_path.join("Contents").join("Info.plist");
  let info_value = Value::from_file(&info_path)?;
  let dict = info_value
    .as_dictionary()
    .ok_or_else(|| PlatformError::MissingInfo("Info.plist 结构无效".into()))?;

  dict
    .get("CFBundleIdentifier")
    .and_then(Value::as_string)
    .map(|s| s.to_string())
    .ok_or_else(|| PlatformError::MissingInfo("缺少 CFBundleIdentifier".into()))
}

fn ensure_extension_normalized(ext: &str) -> String {
  ext.trim_start_matches('.').to_lowercase()
}

fn list_file_associations_impl() -> Result<Vec<FileAssociation>, PlatformError> {
  let value = load_launch_services_value()?;
  let handlers = handlers_from_value(&value)?;

  let extensions = load_extension_list()?;

  let mut results = Vec::with_capacity(extensions.len());
  for ext in extensions {
    if let Some(bundle_id) = find_bundle_id_for_extension(handlers, &ext) {
      match bundle_path_from_id(&bundle_id) {
        Ok(path) => {
          let display_name = application_name_from_path(&path).unwrap_or_else(|_| bundle_id.clone());
          results.push(FileAssociation {
            extension: ext.clone(),
            application_name: display_name,
            application_path: path.display().to_string(),
          });
        }
        Err(err) => {
          results.push(FileAssociation {
            extension: ext.clone(),
            application_name: format!("{} (未找到路径)", humanize_bundle_id(&bundle_id)),
            application_path: err.to_string(),
          });
        }
      }
    } else {
      // 尝试通过 LaunchServices 的系统默认关联获取 bundle id
      if let Some(bundle_id) = system_default_bundle_id_for_extension(&ext) {
        match bundle_path_from_id(&bundle_id) {
          Ok(path) => {
            let display_name =
              application_name_from_path(&path).unwrap_or_else(|_| bundle_id.clone());
            results.push(FileAssociation {
              extension: ext.clone(),
              application_name: display_name,
              application_path: path.display().to_string(),
            });
          }
          Err(_) => {
            results.push(FileAssociation {
              extension: ext.clone(),
              application_name: humanize_bundle_id(&bundle_id),
              application_path: String::new(),
            });
          }
        }
      } else {
        results.push(FileAssociation {
          extension: ext.clone(),
          application_name: "未设置默认应用".into(),
          application_path: "".into(),
        });
      }
    }
  }

  Ok(results)
}

fn add_extension_impl(extension: String) -> Result<Vec<FileAssociation>, PlatformError> {
  let normalized = ensure_extension_normalized(&extension);

  if normalized.is_empty() {
    return Err(PlatformError::InvalidSelection(
      "扩展名不能为空".into(),
    ));
  }

  if !normalized
    .chars()
    .all(|ch| ch.is_ascii_alphanumeric() || ch == '+' || ch == '-')
  {
    return Err(PlatformError::InvalidSelection(
      "扩展名只能包含字母、数字、加号或减号".into(),
    ));
  }

  register_extension_if_needed(&normalized)?;
  list_file_associations_impl()
}

fn set_default_application_impl(
  extension: String,
  application_path: String,
) -> Result<(), PlatformError> {
  let normalized = ensure_extension_normalized(&extension);
  let app_path = resolve_app_bundle_path(&application_path)?;

  let bundle_id = bundle_id_from_path(&app_path)?;
  let content_type = extension_to_content_type(&normalized);

  register_extension_if_needed(&normalized)?;

  let mut value = load_launch_services_value()?;
  let handlers = handlers_from_value_mut(&mut value)?;

  upsert_extension_handler(handlers, &normalized, &bundle_id);
  if let Some(content_type) = content_type {
    upsert_content_type_handler(handlers, content_type, &bundle_id);
    set_launchservices_default(content_type, &bundle_id)?;
  } else {
    // 对于没有预定义内容类型的扩展名，尝试使用UTTypeCreatePreferredIdentifierForTag
    set_extension_handler_by_tag(&normalized, &bundle_id)?;
  }

  let path = launch_services_plist_path()?;
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  plist::to_file_xml(path, &value)?;

  // 重启相关服务以使更改生效
  let _ = Command::new("killall").arg("cfprefsd").status();

  Ok(())
}

fn resolve_app_bundle_path(raw_path: &str) -> Result<PathBuf, PlatformError> {
  let trimmed = raw_path.trim();
  let initial = if let Some(url_like) = trimmed.strip_prefix("file://") {
    if trimmed.starts_with("file:///") {
      Url::parse(trimmed)
        .map_err(|err| PlatformError::InvalidSelection(err.to_string()))?
        .to_file_path()
        .map_err(|_| PlatformError::InvalidSelection(trimmed.to_string()))?
    } else {
      PathBuf::from(url_like)
    }
  } else if trimmed.starts_with("~/") || trimmed == "~" {
    let home = env::var("HOME")?;
    if trimmed == "~" {
      PathBuf::from(home)
    } else {
      PathBuf::from(home).join(&trimmed[2..])
    }
  } else {
    PathBuf::from(trimmed)
  };

  let expanded = fs::canonicalize(&initial).unwrap_or(initial);

  if !expanded.exists() {
    return Err(PlatformError::InvalidSelection(format!(
      "应用路径不存在: {trimmed}"
    )));
  }

  // If the user picked a binary inside the bundle, walk up to the enclosing *.app directory.
  let app_bundle = expanded
    .ancestors()
    .find(|ancestor| {
      ancestor
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("app"))
        .unwrap_or(false)
    })
    .map(Path::to_path_buf);

  let bundle_path = if let Some(path) = app_bundle {
    path
  } else {
    return Err(PlatformError::InvalidSelection(format!(
      "请选择有效的 .app 包: {raw_path}"
    )));
  };

  Ok(bundle_path)
}

fn upsert_extension_handler(
  handlers: &mut Vec<Value>,
  extension: &str,
  bundle_id: &str,
) {
  for handler in handlers.iter_mut() {
    if let Value::Dictionary(dict) = handler {
      let tag = dict
        .get("LSHandlerContentTag")
        .and_then(Value::as_string)
        .map(str::to_lowercase);
      let tag_class = dict
        .get("LSHandlerContentTagClass")
        .and_then(Value::as_string);

      if tag.as_deref() == Some(extension) && tag_class == Some("public.filename-extension") {
        dict.insert(
          "LSHandlerRoleAll".to_string(),
          Value::String(bundle_id.to_string()),
        );
        return;
      }
    }
  }

  let mut new_dict = Dictionary::new();
  new_dict.insert(
    "LSHandlerContentTag".to_string(),
    Value::String(extension.to_string()),
  );
  new_dict.insert(
    "LSHandlerContentTagClass".to_string(),
    Value::String("public.filename-extension".into()),
  );
  new_dict.insert(
    "LSHandlerRoleAll".to_string(),
    Value::String(bundle_id.to_string()),
  );
  handlers.push(Value::Dictionary(new_dict));
}

fn upsert_content_type_handler(
  handlers: &mut Vec<Value>,
  content_type: &str,
  bundle_id: &str,
) {
  for handler in handlers.iter_mut() {
    if let Value::Dictionary(dict) = handler {
      let handler_content_type = dict.get("LSHandlerContentType").and_then(Value::as_string);
      if handler_content_type.as_deref() == Some(content_type) {
        dict.insert(
          "LSHandlerRoleAll".to_string(),
          Value::String(bundle_id.to_string()),
        );
        return;
      }
    }
  }

  let mut new_dict = Dictionary::new();
  new_dict.insert(
    "LSHandlerContentType".to_string(),
    Value::String(content_type.to_string()),
  );
  new_dict.insert(
    "LSHandlerRoleAll".to_string(),
    Value::String(bundle_id.to_string()),
  );
  handlers.push(Value::Dictionary(new_dict));
}

fn extension_to_content_type(ext: &str) -> Option<&'static str> {
  EXTENSION_TO_CONTENT_TYPE
    .iter()
    .find(|(key, _)| key.eq_ignore_ascii_case(ext))
    .map(|(_, value)| *value)
}

fn humanize_bundle_id(bundle_id: &str) -> String {
  // Use the last component after '.' and insert spaces at camel/digit boundaries
  let core = bundle_id.rsplit('.').next().unwrap_or(bundle_id);
  let s = core.replace('_', " ").replace('-', " ");
  let mut result = String::new();
  let mut prev: Option<char> = None;
  for ch in s.chars() {
    if let Some(p) = prev {
      let boundary =
        (p.is_ascii_lowercase() && ch.is_ascii_uppercase()) ||
        (p.is_ascii_alphabetic() && ch.is_ascii_digit()) ||
        (p.is_ascii_digit() && ch.is_ascii_alphabetic());
      if boundary && !result.ends_with(' ') {
        result.push(' ');
      }
    }
    result.push(ch);
    prev = Some(ch);
  }
  result
}

fn copy_default_handler_for_content_type(content_type: &str) -> Option<String> {
  let content_c = CString::new(content_type).ok()?;
  unsafe {
    let content_cf =
      CFStringCreateWithCString(kCFAllocatorDefault, content_c.as_ptr(), CFSTRING_ENCODING_UTF8);
    if content_cf.is_null() {
      return None;
    }
    let handler_cf = LSCopyDefaultRoleHandlerForContentType(content_cf, LS_ROLES_ALL);
    CFRelease(content_cf);
    if handler_cf.is_null() {
      return None;
    }
    let mut buf = vec![0u8; 1024];
    let ok = CFStringGetCString(
      handler_cf,
      buf.as_mut_ptr() as *mut c_char,
      buf.len() as isize,
      CFSTRING_ENCODING_UTF8,
    );
    CFRelease(handler_cf);
    if ok != 0 {
      let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
      String::from_utf8(buf[..len].to_vec()).ok()
    } else {
      None
    }
  }
}

fn system_default_bundle_id_for_extension(ext: &str) -> Option<String> {
  if let Some(content_type) = extension_to_content_type(ext) {
    copy_default_handler_for_content_type(content_type)
  } else {
    let generic = format!("public.{}", ext);
    copy_default_handler_for_content_type(&generic)
  }
}

const LS_ROLES_ALL: u32 = 0xFFFFFFFF;

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
  fn LSSetDefaultRoleHandlerForContentType(
    in_content_type: CFStringRef,
    in_role: u32,
    in_bundle_identifier: CFStringRef,
  ) -> i32;
  fn LSCopyDefaultRoleHandlerForContentType(
    in_content_type: CFStringRef,
    in_role: u32,
  ) -> CFStringRef;
}

fn set_launchservices_default(content_type: &str, bundle_id: &str) -> Result<(), PlatformError> {
  let content_c = CString::new(content_type)
    .map_err(|_| PlatformError::InvalidSelection(format!("非法的内容类型: {content_type}")))?;
  let bundle_c = CString::new(bundle_id)
    .map_err(|_| PlatformError::InvalidSelection(format!("非法的应用 ID: {bundle_id}")))?;

  unsafe {
    let content_cf =
      CFStringCreateWithCString(kCFAllocatorDefault, content_c.as_ptr(), CFSTRING_ENCODING_UTF8);
    let bundle_cf =
      CFStringCreateWithCString(kCFAllocatorDefault, bundle_c.as_ptr(), CFSTRING_ENCODING_UTF8);

    if content_cf.is_null() || bundle_cf.is_null() {
      if !content_cf.is_null() {
        CFRelease(content_cf);
      }
      if !bundle_cf.is_null() {
        CFRelease(bundle_cf);
      }
      return Err(PlatformError::Command(
        "创建 CFString 失败，无法更新 LaunchServices".into(),
      ));
    }

    let status =
      LSSetDefaultRoleHandlerForContentType(content_cf, LS_ROLES_ALL, bundle_cf);

    CFRelease(content_cf);
    CFRelease(bundle_cf);

    if status == 0 {
      Ok(())
    } else {
      Err(PlatformError::Command(format!(
        "LSSetDefaultRoleHandlerForContentType 失败: {status}"
      )))
    }
  }
}

fn set_extension_handler_by_tag(extension: &str, bundle_id: &str) -> Result<(), PlatformError> {
  // 尝试使用duti命令设置，这是macOS推荐的命令行工具
  let output = Command::new("duti")
    .arg("-s")
    .arg(bundle_id)
    .arg(extension)
    .arg("all")
    .output();

  match output {
    Ok(result) => {
      if result.status.success() {
        eprintln!("使用 duti 成功设置 .{} 的默认应用为 {}", extension, bundle_id);
        Ok(())
      } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        eprintln!("duti 命令失败: {}, 尝试备用方法", stderr);
        // 如果duti失败，尝试直接使用LS API
        set_extension_directly(extension, bundle_id)
      }
    }
    Err(err) => {
      eprintln!("无法执行 duti 命令: {}, 尝试备用方法", err);
      // 如果duti不可用，尝试直接使用LS API
      set_extension_directly(extension, bundle_id)
    }
  }
}

fn set_extension_directly(extension: &str, bundle_id: &str) -> Result<(), PlatformError> {
  // 尝试创建一个动态的内容类型
  let content_type = format!("public.{}", extension);

  let content_c = CString::new(content_type.as_str())
    .map_err(|_| PlatformError::InvalidSelection(format!("非法的内容类型: {content_type}")))?;
  let bundle_c = CString::new(bundle_id)
    .map_err(|_| PlatformError::InvalidSelection(format!("非法的应用 ID: {bundle_id}")))?;

  unsafe {
    let content_cf =
      CFStringCreateWithCString(kCFAllocatorDefault, content_c.as_ptr(), CFSTRING_ENCODING_UTF8);
    let bundle_cf =
      CFStringCreateWithCString(kCFAllocatorDefault, bundle_c.as_ptr(), CFSTRING_ENCODING_UTF8);

    if content_cf.is_null() || bundle_cf.is_null() {
      if !content_cf.is_null() {
        CFRelease(content_cf);
      }
      if !bundle_cf.is_null() {
        CFRelease(bundle_cf);
      }
      return Err(PlatformError::Command(
        "创建 CFString 失败，无法更新 LaunchServices".into(),
      ));
    }

    let status =
      LSSetDefaultRoleHandlerForContentType(content_cf, LS_ROLES_ALL, bundle_cf);

    CFRelease(content_cf);
    CFRelease(bundle_cf);

    if status == 0 {
      eprintln!("使用 LS API 成功设置 .{} 的默认应用为 {}", extension, bundle_id);
      Ok(())
    } else {
      eprintln!("LS API 设置失败: {}, 将仅依赖 plist 配置", status);
      // 即使LS API失败，我们已经设置了plist配置，所以返回Ok
      Ok(())
    }
  }
}
