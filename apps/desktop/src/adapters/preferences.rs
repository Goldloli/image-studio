//! Application-level preferences, intentionally independent from the project library.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

pub const PREFERENCES_SCHEMA_VERSION: u32 = 1;
const PREFERENCES_FILE: &str = "application-preferences.json";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearancePreferences {
    pub theme: String,
    pub font_scale: String,
    pub density: String,
    pub reduced_motion: bool,
    pub sidebar_collapsed: bool,
}

impl Default for AppearancePreferences {
    fn default() -> Self {
        Self {
            theme: "light".into(),
            font_scale: "default".into(),
            density: "comfortable".into(),
            reduced_motion: false,
            sidebar_collapsed: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreationDefaults {
    pub model_id: String,
    pub aspect: String,
    pub count: u32,
    pub seed: String,
}

impl Default for CreationDefaults {
    fn default() -> Self {
        Self {
            model_id: "z-image-turbo-mflux-v1".into(),
            aspect: "1:1".into(),
            count: 1,
            seed: String::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreferencesDocument {
    schema_version: u32,
    appearance: AppearancePreferences,
    creation_defaults: CreationDefaults,
}

impl Default for PreferencesDocument {
    fn default() -> Self {
        Self {
            schema_version: PREFERENCES_SCHEMA_VERSION,
            appearance: AppearancePreferences::default(),
            creation_defaults: CreationDefaults::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesSnapshot {
    pub schema_version: u32,
    pub appearance: AppearancePreferences,
    pub creation_defaults: CreationDefaults,
    pub load_error: Option<String>,
    pub save_error: Option<String>,
    pub warnings: Vec<String>,
}

impl PreferencesSnapshot {
    fn defaults() -> Self {
        let document = PreferencesDocument::default();
        Self {
            schema_version: document.schema_version,
            appearance: document.appearance,
            creation_defaults: document.creation_defaults,
            load_error: None,
            save_error: None,
            warnings: Vec::new(),
        }
    }

    fn document(&self) -> PreferencesDocument {
        PreferencesDocument {
            schema_version: PREFERENCES_SCHEMA_VERSION,
            appearance: self.appearance.clone(),
            creation_defaults: self.creation_defaults.clone(),
        }
    }
}

pub struct PreferencesStore {
    path: PathBuf,
    state: Mutex<PreferencesSnapshot>,
}

impl PreferencesStore {
    pub fn load(data_root: &Path) -> Self {
        let path = data_root.join(PREFERENCES_FILE);
        let state = read_preferences(&path);
        Self {
            path,
            state: Mutex::new(state),
        }
    }

    pub fn snapshot(&self) -> PreferencesSnapshot {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn reload(&self) -> PreferencesSnapshot {
        let next = read_preferences(&self.path);
        *self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = next.clone();
        next
    }

    pub fn save_appearance(
        &self,
        appearance: AppearancePreferences,
    ) -> Result<PreferencesSnapshot, String> {
        validate_appearance(&appearance)?;
        self.save(|current| current.appearance = appearance.clone())
    }

    pub fn save_creation_defaults(
        &self,
        creation_defaults: CreationDefaults,
    ) -> Result<PreferencesSnapshot, String> {
        validate_creation_defaults(&creation_defaults)?;
        self.save(|current| current.creation_defaults = creation_defaults.clone())
    }

    fn save(
        &self,
        update: impl FnOnce(&mut PreferencesDocument),
    ) -> Result<PreferencesSnapshot, String> {
        let mut guard = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut document = guard.document();
        update(&mut document);
        document.schema_version = PREFERENCES_SCHEMA_VERSION;

        // Keep the accepted values in memory even if persistence fails, but never
        // present them as saved.
        guard.appearance = document.appearance.clone();
        guard.creation_defaults = document.creation_defaults.clone();
        let result = write_atomic(&self.path, &document);
        match result {
            Ok(()) => {
                guard.load_error = None;
                guard.save_error = None;
                guard.warnings.clear();
                Ok(guard.clone())
            }
            Err(error) => {
                guard.save_error = Some(error.clone());
                Err(error)
            }
        }
    }
}

pub fn validate_appearance(value: &AppearancePreferences) -> Result<(), String> {
    let mut errors = Vec::new();
    if !matches!(value.theme.as_str(), "light" | "dark" | "system") {
        errors.push("主题必须是 light、dark 或 system");
    }
    if !matches!(value.font_scale.as_str(), "small" | "default" | "large") {
        errors.push("字号必须是 small、default 或 large");
    }
    if !matches!(value.density.as_str(), "comfortable" | "compact") {
        errors.push("界面密度必须是 comfortable 或 compact");
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("偏好字段无效：{}", errors.join("；")))
    }
}

pub fn validate_creation_defaults(value: &CreationDefaults) -> Result<(), String> {
    if value.model_id.is_empty()
        || value.model_id.len() > 128
        || !value
            .model_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
    {
        return Err("默认模型标识无效".into());
    }
    if !matches!(value.aspect.as_str(), "1:1" | "4:3" | "3:4") {
        return Err("默认画幅不受支持".into());
    }
    if value.count != 1 {
        return Err("当前创作能力每轮只接受 1 张图片".into());
    }
    if !valid_seed(&value.seed) {
        return Err("默认种子须为 0–4294967295 的整数或留空".into());
    }
    Ok(())
}

pub fn valid_seed(seed: &str) -> bool {
    seed.is_empty()
        || (seed.bytes().all(|byte| byte.is_ascii_digit()) && seed.parse::<u32>().is_ok())
}

fn read_preferences(path: &Path) -> PreferencesSnapshot {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return PreferencesSnapshot::defaults();
        }
        Err(error) => {
            let mut fallback = PreferencesSnapshot::defaults();
            fallback.load_error = Some(format!("无法读取应用偏好 ({path:?})：{error}"));
            return fallback;
        }
    };
    if !metadata.file_type().is_file() {
        let mut fallback = PreferencesSnapshot::defaults();
        fallback.load_error = Some("应用偏好路径不是普通文件，未读取或覆盖。".into());
        return fallback;
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            let mut fallback = PreferencesSnapshot::defaults();
            fallback.load_error = Some(format!("无法读取应用偏好 ({path:?})：{error}"));
            return fallback;
        }
    };
    let value: Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(error) => {
            let mut fallback = PreferencesSnapshot::defaults();
            fallback.load_error = Some(format!("应用偏好 JSON 无法解析：{error}"));
            return fallback;
        }
    };
    let Some(object) = value.as_object() else {
        let mut fallback = PreferencesSnapshot::defaults();
        fallback.load_error = Some("应用偏好必须是 JSON 对象。".into());
        return fallback;
    };
    if object.get("schemaVersion").and_then(Value::as_u64)
        != Some(u64::from(PREFERENCES_SCHEMA_VERSION))
    {
        let mut fallback = PreferencesSnapshot::defaults();
        fallback.load_error = Some(format!(
            "应用偏好版本不受支持或缺失（期望 {PREFERENCES_SCHEMA_VERSION}）。"
        ));
        return fallback;
    }

    let mut warnings = Vec::new();
    let appearance_value = object.get("appearance").and_then(Value::as_object);
    let defaults_value = object.get("creationDefaults").and_then(Value::as_object);
    let mut defaults = AppearancePreferences::default();
    if let Some(values) = appearance_value {
        defaults.theme = string_field(
            values,
            "theme",
            &["light", "dark", "system"],
            "light",
            "theme",
            &mut warnings,
        );
        defaults.font_scale = string_field(
            values,
            "fontScale",
            &["small", "default", "large"],
            "default",
            "字号",
            &mut warnings,
        );
        defaults.density = string_field(
            values,
            "density",
            &["comfortable", "compact"],
            "comfortable",
            "界面密度",
            &mut warnings,
        );
        defaults.reduced_motion =
            bool_field(values, "reducedMotion", false, "减弱动效", &mut warnings);
        defaults.sidebar_collapsed =
            bool_field(values, "sidebarCollapsed", false, "侧栏状态", &mut warnings);
    } else {
        warnings.push("appearance 缺失，已使用内置外观".into());
    }
    let mut creation = CreationDefaults::default();
    if let Some(values) = defaults_value {
        creation.model_id = valid_model_id(values.get("modelId")).unwrap_or_else(|| {
            warnings.push("默认模型字段无效，已使用 Z-Image Turbo".into());
            CreationDefaults::default().model_id
        });
        creation.aspect = string_field(
            values,
            "aspect",
            &["1:1", "4:3", "3:4"],
            "1:1",
            "默认画幅",
            &mut warnings,
        );
        creation.count = match values.get("count").and_then(Value::as_u64) {
            Some(1) => 1,
            Some(_) | None => {
                if values.get("count").is_some() {
                    warnings.push("默认数量无效，已回退为 1".into());
                }
                1
            }
        };
        creation.seed = match values.get("seed").and_then(Value::as_str) {
            Some(seed) if valid_seed(seed) => seed.to_owned(),
            Some(_) => {
                warnings.push("默认种子无效，已回退为随机".into());
                String::new()
            }
            None => String::new(),
        };
    } else {
        warnings.push("creationDefaults 缺失，已使用内置创作默认值".into());
    }

    PreferencesSnapshot {
        schema_version: PREFERENCES_SCHEMA_VERSION,
        appearance: defaults,
        creation_defaults: creation,
        load_error: None,
        save_error: None,
        warnings,
    }
}

fn string_field(
    values: &serde_json::Map<String, Value>,
    key: &str,
    allowed: &[&str],
    fallback: &str,
    label: &str,
    warnings: &mut Vec<String>,
) -> String {
    match values.get(key).and_then(Value::as_str) {
        Some(value) if allowed.contains(&value) => value.to_owned(),
        Some(_) => {
            warnings.push(format!("{label}无效，已回退为 {fallback}"));
            fallback.to_owned()
        }
        None => fallback.to_owned(),
    }
}

fn bool_field(
    values: &serde_json::Map<String, Value>,
    key: &str,
    fallback: bool,
    label: &str,
    warnings: &mut Vec<String>,
) -> bool {
    match values.get(key).and_then(Value::as_bool) {
        Some(value) => value,
        None if values.get(key).is_some() => {
            warnings.push(format!("{label}无效，已回退"));
            fallback
        }
        None => fallback,
    }
}

fn valid_model_id(value: Option<&Value>) -> Option<String> {
    let value = value?.as_str()?;
    (!value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte)))
    .then(|| value.to_owned())
}

fn write_atomic(path: &Path, document: &PreferencesDocument) -> Result<(), String> {
    let parent = path.parent().ok_or("应用偏好目录不可用")?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建应用偏好目录：{error}"))?;
    if path.exists() {
        let metadata =
            fs::symlink_metadata(path).map_err(|error| format!("无法检查应用偏好目标：{error}"))?;
        if !metadata.file_type().is_file() {
            return Err("应用偏好目标不是普通文件，未覆盖".into());
        }
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(
        ".application-preferences.{}.{stamp}.tmp",
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(document)
        .map_err(|error| format!("无法序列化应用偏好：{error}"))?;
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("无法创建应用偏好临时文件：{error}"))?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("无法写入应用偏好：{error}"))?;
        fs::rename(&temporary, path).map_err(|error| format!("无法提交应用偏好：{error}"))?;
        if let Ok(directory) = fs::File::open(parent) {
            directory
                .sync_all()
                .map_err(|error| format!("无法同步应用偏好目录：{error}"))?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_uses_defaults_without_writing() {
        let root = tempfile::tempdir().unwrap();
        let store = PreferencesStore::load(root.path());
        let snapshot = store.snapshot();
        assert_eq!(snapshot.appearance, AppearancePreferences::default());
        assert_eq!(snapshot.creation_defaults, CreationDefaults::default());
        assert!(!root.path().join(PREFERENCES_FILE).exists());
    }

    #[test]
    fn malformed_fields_fall_back_individually_and_preserve_other_values() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join(PREFERENCES_FILE),
            r#"{
              "schemaVersion": 1,
              "appearance": {
                "theme": "dark",
                "fontScale": "giant",
                "density": "compact",
                "reducedMotion": true,
                "sidebarCollapsed": false
              },
              "creationDefaults": {
                "modelId": "z-image-turbo-mflux-v1",
                "aspect": "16:9",
                "count": 2,
                "seed": "nope"
              }
            }"#,
        )
        .unwrap();
        let snapshot = PreferencesStore::load(root.path()).snapshot();
        assert_eq!(snapshot.appearance.theme, "dark");
        assert_eq!(snapshot.appearance.font_scale, "default");
        assert_eq!(snapshot.appearance.density, "compact");
        assert!(snapshot.appearance.reduced_motion);
        assert_eq!(snapshot.creation_defaults.aspect, "1:1");
        assert_eq!(snapshot.creation_defaults.count, 1);
        assert_eq!(snapshot.creation_defaults.seed, "");
        assert_eq!(snapshot.warnings.len(), 4);
        assert!(snapshot.load_error.is_none());
    }

    #[test]
    fn save_is_versioned_atomic_and_reloadable() {
        let root = tempfile::tempdir().unwrap();
        let store = PreferencesStore::load(root.path());
        let appearance = AppearancePreferences {
            theme: "dark".into(),
            density: "compact".into(),
            ..AppearancePreferences::default()
        };
        store.save_appearance(appearance).unwrap();
        let contents = fs::read_to_string(root.path().join(PREFERENCES_FILE)).unwrap();
        let value: Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        let loaded = PreferencesStore::load(root.path()).snapshot();
        assert_eq!(loaded.appearance.theme, "dark");
        assert_eq!(loaded.appearance.density, "compact");
    }

    #[test]
    fn corrupt_file_is_reported_and_not_overwritten_on_load() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join(PREFERENCES_FILE), b"{bad").unwrap();
        let snapshot = PreferencesStore::load(root.path()).snapshot();
        assert!(snapshot.load_error.is_some());
        assert_eq!(
            fs::read(root.path().join(PREFERENCES_FILE)).unwrap(),
            b"{bad"
        );
    }
}
