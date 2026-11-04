use std::path::{Path, PathBuf};

/// Windows legacy `MAX_PATH` is 260 UTF-16 units. The frozen shared runtime
/// keeps its longest load-relevant member at 86 units and stages with at most a
/// 26-unit directory name, plus the `\runtimes\` prefix (10) and a separator;
/// rejecting data paths above this limit leaves a 20-unit safety margin before
/// onnxruntime/Python's non-long-path DLL loading can fail, and happens before
/// any download or extraction starts.
#[cfg(windows)]
pub const MAX_DATA_PATH_UNITS: usize = 117;

#[cfg(windows)]
const DATA_ROOT_SUGGESTION: &str = r"C:\ImageStudioData";

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("无法解析应用目录：{0}。请使用 --data-dir 指定可写目录。")]
    MissingLocation(&'static str),
    #[cfg(windows)]
    #[error(
        "无法解析 Windows 本地应用数据目录（HRESULT 0x{0:08X}）。请使用 --data-dir 指定可写目录。"
    )]
    WindowsKnownFolder(i32),
    #[cfg(windows)]
    #[error(
        "数据目录 {path} 路径过长（{length} 个字符，Windows 运行时限 {limit}）。请把数据目录移到更短的位置，例如 {suggestion}，或用 --data-dir 指定短目录。"
    )]
    DataPathTooLong {
        path: PathBuf,
        length: usize,
        limit: usize,
        suggestion: &'static str,
    },
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
        #[cfg(windows)]
        validate_data_path_length(&paths.data)?;
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
fn windows_path_units(path: &Path) -> usize {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().count()
}

#[cfg(windows)]
fn validate_data_path_length(data: &Path) -> Result<(), PlatformError> {
    let absolute = std::path::absolute(data).map_err(|source| PlatformError::Directory {
        path: data.to_path_buf(),
        source,
    })?;
    let length = windows_path_units(&absolute);
    if length > MAX_DATA_PATH_UNITS {
        return Err(PlatformError::DataPathTooLong {
            path: absolute,
            length,
            limit: MAX_DATA_PATH_UNITS,
            suggestion: DATA_ROOT_SUGGESTION,
        });
    }
    Ok(())
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

    #[cfg(windows)]
    fn path_with_units(units: usize) -> PathBuf {
        // `C:\` is three UTF-16 units; the single component adds no separators.
        assert!(units >= 3);
        PathBuf::from(r"C:\").join("a".repeat(units - 3))
    }

    #[cfg(windows)]
    #[test]
    fn windows_data_path_guard_accepts_the_boundary_and_rejects_one_more_unit() {
        assert!(validate_data_path_length(Path::new(r"C:\ist-a\data")).is_ok());
        assert!(validate_data_path_length(&path_with_units(MAX_DATA_PATH_UNITS)).is_ok());
        let error = validate_data_path_length(&path_with_units(MAX_DATA_PATH_UNITS + 1))
            .expect_err("one unit over the limit must be rejected");
        let message = error.to_string();
        assert!(message.contains("路径过长"), "{message}");
        assert!(message.contains("--data-dir"), "{message}");
        assert!(message.contains("ImageStudioData"), "{message}");
    }

    #[cfg(windows)]
    #[test]
    fn windows_data_path_guard_counts_utf16_code_units() {
        use std::os::windows::ffi::OsStrExt;
        let mut path = path_with_units(MAX_DATA_PATH_UNITS - 3);
        path.push("😀"); // one separator + a surrogate pair = three UTF-16 units
        assert_eq!(path.as_os_str().encode_wide().count(), MAX_DATA_PATH_UNITS);
        assert!(validate_data_path_length(&path).is_ok());
        path.push("x");
        assert_eq!(
            path.as_os_str().encode_wide().count(),
            MAX_DATA_PATH_UNITS + 2
        );
        assert!(validate_data_path_length(&path).is_err());
    }
}
