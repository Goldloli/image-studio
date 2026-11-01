//! Read-only storage projections and host-owned directory reveal actions.
use super::paths::AppPaths;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug)]
struct Category {
    id: &'static str,
    label: &'static str,
    kind: CategoryKind,
}

#[derive(Clone, Copy, Debug)]
enum CategoryKind {
    Data,
    Models,
    Runtimes,
    Cache,
    Images,
    Logs,
}

const CATEGORIES: &[Category] = &[
    Category {
        id: "data",
        label: "应用数据",
        kind: CategoryKind::Data,
    },
    Category {
        id: "models",
        label: "模型",
        kind: CategoryKind::Models,
    },
    Category {
        id: "runtimes",
        label: "运行环境",
        kind: CategoryKind::Runtimes,
    },
    Category {
        id: "cache",
        label: "下载缓存",
        kind: CategoryKind::Cache,
    },
    Category {
        id: "images",
        label: "生成图片",
        kind: CategoryKind::Images,
    },
    Category {
        id: "logs",
        label: "日志",
        kind: CategoryKind::Logs,
    },
];

impl CategoryKind {
    fn path(self, paths: &AppPaths) -> PathBuf {
        match self {
            Self::Data => paths.data.clone(),
            Self::Models => paths.data.join("models"),
            Self::Runtimes => paths.data.join("runtimes"),
            Self::Cache => paths.cache.clone(),
            Self::Images => paths.data.join("assets"),
            Self::Logs => paths.logs.clone(),
        }
    }
}

pub fn summary(paths: &AppPaths) -> Value {
    let entries = CATEGORIES
        .iter()
        .map(|category| {
            let path = category.kind.path(paths);
            let (bytes, status, error) = match tree_size(&path) {
                Ok(bytes) => (bytes, "ready", None),
                Err((kind, bytes, message)) => {
                    (bytes, kind, (!message.is_empty()).then_some(message))
                }
            };
            let available = if path.exists() {
                fs2::available_space(&path).ok()
            } else {
                None
            };
            json!({
                "id": category.id,
                "label": category.label,
                "path": path,
                "bytes": bytes,
                "availableBytes": available,
                "status": status,
                "error": error
            })
        })
        .collect::<Vec<_>>();
    json!({
        "entries": entries,
        "migrationSupported": false,
        "deletionSupported": false
    })
}

pub fn open_location(paths: &AppPaths, id: &str) -> Result<(), String> {
    let category = CATEGORIES
        .iter()
        .find(|category| category.id == id)
        .ok_or("未登记的存储位置")?;
    let path = category.kind.path(paths);
    if !path.is_dir() {
        return Err(format!("{}目录尚未创建", category.label));
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let status = Command::new("open")
            .arg(&path)
            .status()
            .map_err(|error| format!("无法打开{}目录：{error}", category.label))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("系统未能打开{}目录", category.label))
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err("本阶段仅验证 macOS 目录定位".into())
    }
}

fn tree_size(path: &Path) -> Result<u64, (&'static str, u64, String)> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(("missing", 0, String::new()));
        }
        Err(error) => return Err(("error", 0, format!("无法读取目录元数据：{error}"))),
    };
    if metadata.file_type().is_symlink() {
        return Err(("error", 0, "登记目录是符号链接，未跟随读取".into()));
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Err(("error", 0, "登记路径不是普通文件或目录".into()));
    }
    let mut total = 0_u64;
    let mut errors = Vec::new();
    size_directory(path, &mut total, &mut errors, 0);
    if errors.is_empty() {
        Ok(total)
    } else {
        Err((
            "partial",
            total,
            format!("部分内容无法读取：{}", errors.join("；")),
        ))
    }
}

fn size_directory(path: &Path, total: &mut u64, errors: &mut Vec<String>, depth: usize) {
    if depth > 64 {
        errors.push(format!("目录层级过深：{}", path.display()));
        return;
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(format!("{}：{error}", path.display()));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                errors.push(format!("{}：{error}", path.display()));
                continue;
            }
        };
        let entry_path = entry.path();
        let metadata = match fs::symlink_metadata(&entry_path) {
            Ok(metadata) => metadata,
            Err(error) => {
                errors.push(format!("{}：{error}", entry_path.display()));
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            size_directory(&entry_path, total, errors, depth + 1);
        } else if metadata.is_file() {
            *total = total.saturating_add(metadata.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_reports_real_files_and_missing_directories_without_hashing() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(root.path())).unwrap();
        fs::create_dir_all(paths.cache.join("downloads")).unwrap();
        fs::write(paths.cache.join("downloads/a"), b"12345").unwrap();
        let value = summary(&paths);
        let cache = value["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == "cache")
            .unwrap();
        assert_eq!(cache["bytes"].as_u64(), Some(5));
        assert_eq!(cache["status"], "ready");
        let models = value["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == "models")
            .unwrap();
        assert_eq!(models["status"], "missing");
        assert_eq!(models["bytes"].as_u64(), Some(0));
    }

    #[test]
    fn arbitrary_paths_are_not_eligible_for_open() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(root.path())).unwrap();
        assert!(open_location(&paths, "/tmp").is_err());
    }
}
