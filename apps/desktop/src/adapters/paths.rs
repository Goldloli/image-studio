use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("无法解析应用目录：{0}。请使用 --data-dir 指定可写目录。")]
    MissingLocation(&'static str),
    #[cfg(windows)]
    #[error(
        "无法解析 Windows 本地应用数据目录（HRESULT 0x{0:08X}）。请使用 --data-dir 指定可写目录。"
    )]
    WindowsKnownFolder(i32),
    #[error("无法创建或写入目录 {path}: {source}。请检查权限或使用 --data-dir 更换位置。")]
    Directory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub data: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
    pub work: PathBuf,
}

impl AppPaths {
    pub fn resolve(root: Option<&Path>) -> Result<Self, PlatformError> {
        let mut paths = if let Some(root) = root {
            Self {
                data: root.join("data"),
                cache: root.join("cache"),
                logs: root.join("logs"),
                work: root.join("work"),
            }
        } else {
            Self::system_paths()?
        };
        for path in [
            &mut paths.data,
            &mut paths.cache,
            &mut paths.logs,
            &mut paths.work,
        ] {
            std::fs::create_dir_all(&*path).map_err(|source| PlatformError::Directory {
                path: path.clone(),
                source,
            })?;
            // A real create/remove probe catches unwritable existing directories too.
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let probe = path.join(format!(".write-probe-{}-{stamp}", std::process::id()));
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&probe)
                .map_err(|source| PlatformError::Directory {
                    path: path.clone(),
                    source,
                })?;
            drop(file);
            std::fs::remove_file(&probe).map_err(|source| PlatformError::Directory {
                path: path.clone(),
                source,
            })?;
            *path = std::fs::canonicalize(&*path).map_err(|source| PlatformError::Directory {
                path: path.clone(),
                source,
            })?;
        }
        Ok(paths)
    }

    #[cfg(target_os = "macos")]
    fn system_paths() -> Result<Self, PlatformError> {
        let home = std::env::var_os("HOME").ok_or(PlatformError::MissingLocation("HOME"))?;
        let library = PathBuf::from(home).join("Library");
        let data = library.join("Application Support/ImageStudio");
        Ok(Self {
            work: data.join("diagnostics"),
            data,
            cache: library.join("Caches/ImageStudio"),
            logs: library.join("Logs/ImageStudio"),
        })
    }

    #[cfg(windows)]
    fn system_paths() -> Result<Self, PlatformError> {
        let root = windows_local_app_data()?.join("ImageStudio");
        Ok(Self {
            data: root.join("data"),
            cache: root.join("cache"),
            logs: root.join("logs"),
            work: root.join("work"),
        })
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    fn system_paths() -> Result<Self, PlatformError> {
        Err(PlatformError::MissingLocation("当前平台默认目录尚未实现"))
    }
}

#[cfg(windows)]
fn windows_local_app_data() -> Result<PathBuf, PlatformError> {
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath};

    let mut raw = std::ptr::null_mut();
    // SAFETY: the shell allocates the returned UTF-16 string with CoTaskMemAlloc;
    // it is copied below and released exactly once with CoTaskMemFree.
    let result =
        unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, std::ptr::null_mut(), &mut raw) };
    if result < 0 {
        return Err(PlatformError::WindowsKnownFolder(result));
    }
    if raw.is_null() {
        return Err(PlatformError::MissingLocation("LocalAppData"));
    }
    let mut length = 0usize;
    // SAFETY: SHGetKnownFolderPath returned a NUL-terminated UTF-16 allocation.
    unsafe {
        while *raw.add(length) != 0 {
            length += 1;
        }
    }
    // SAFETY: length is bounded by the NUL terminator in the returned allocation.
    let path = String::from_utf16(unsafe { std::slice::from_raw_parts(raw, length) })
        .map(PathBuf::from)
        .map_err(|_| PlatformError::MissingLocation("LocalAppData"));
    // SAFETY: raw is the original allocation returned by the API and is not used again.
    unsafe { CoTaskMemFree(raw.cast()) };
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_spaces_and_shell_characters_are_literal_paths() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("中文 空格 ; $(never-run)");
        let first = AppPaths::resolve(Some(&root)).unwrap();
        let second = AppPaths::resolve(Some(&root)).unwrap();
        assert_eq!(first.data, second.data);
        for path in [first.data, first.cache, first.logs, first.work] {
            assert!(path.is_dir());
            assert_eq!(std::fs::read_dir(path).unwrap().count(), 0);
        }
    }

    #[test]
    fn occupied_directory_reports_action_without_overwriting() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("data"), b"keep").unwrap();
        let error = AppPaths::resolve(Some(temp.path())).unwrap_err();
        assert!(error.to_string().contains("--data-dir"));
        assert_eq!(std::fs::read(temp.path().join("data")).unwrap(), b"keep");
    }

    #[cfg(windows)]
    #[test]
    fn windows_local_app_data_is_an_absolute_known_folder() {
        let path = windows_local_app_data().unwrap();
        assert!(path.is_absolute());
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("Local")
        );
    }
}
