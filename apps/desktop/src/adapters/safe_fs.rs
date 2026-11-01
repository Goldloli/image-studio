use std::{
    fs::{self, File, Metadata, OpenOptions},
    io,
    path::Path,
};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

pub(crate) fn open_regular(
    path: &Path,
    write: bool,
    create: bool,
    truncate: bool,
) -> io::Result<File> {
    reject_windows_alternate_stream(path)?;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(write)
        .create(create)
        .truncate(truncate);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }

    let file = options.open(path)?;
    validate_regular(&file.metadata()?)?;
    Ok(file)
}

#[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
pub(crate) fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(not(windows))]
    {
        fs::rename(from, to)
    }
    #[cfg(windows)]
    {
        let original_permissions = match fs::symlink_metadata(to) {
            Ok(metadata) if metadata.is_file() => Some(metadata.permissions()),
            Ok(_) => {
                return Err(invalid("替换目标不是普通文件"));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        if let Some(permissions) = original_permissions.as_ref()
            && permissions.readonly()
        {
            let mut writable = permissions.clone();
            writable.set_readonly(false);
            fs::set_permissions(to, writable)?;
        }
        match fs::rename(from, to) {
            Ok(()) => Ok(()),
            Err(error) => {
                if let Some(permissions) = original_permissions {
                    let _ = fs::set_permissions(to, permissions);
                }
                Err(error)
            }
        }
    }
}

fn validate_regular(metadata: &Metadata) -> io::Result<()> {
    if !metadata.is_file() {
        return Err(invalid("管理文件不是普通文件"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(invalid("管理文件不能是重解析点"));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn reject_windows_alternate_stream(path: &Path) -> io::Result<()> {
    let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let prefixed = units.starts_with(&[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16])
        || units.starts_with(&[b'\\' as u16, b'\\' as u16, b'.' as u16, b'\\' as u16]);
    for (index, unit) in units.iter().enumerate() {
        if *unit != b':' as u16 {
            continue;
        }
        let drive_index = if prefixed { 4 } else { 0 };
        if index != drive_index + 1
            || !units.get(drive_index).is_some_and(|value| {
                char::from_u32(u32::from(*value)).is_some_and(|c| c.is_ascii_alphabetic())
            })
        {
            return Err(invalid("管理文件路径不能包含备用数据流"));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn reject_windows_alternate_stream(_: &Path) -> io::Result<()> {
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_file_opens_for_reading() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("fixture.bin");
        std::fs::write(&path, b"fixture").unwrap();
        let mut file = open_regular(&path, false, false, false).unwrap();
        let mut bytes = String::new();
        std::io::Read::read_to_string(&mut file, &mut bytes).unwrap();
        assert_eq!(bytes, "fixture");
    }

    #[cfg(windows)]
    #[test]
    fn alternate_data_stream_is_rejected_before_open() {
        let root = tempfile::tempdir().unwrap();
        let error =
            open_regular(&root.path().join("fixture:stream"), true, true, true).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("备用数据流"));
    }

    #[cfg(windows)]
    #[test]
    fn replace_file_handles_a_readonly_target() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.tmp");
        let target = root.path().join("target.png");
        std::fs::write(&source, b"new").unwrap();
        std::fs::write(&target, b"old").unwrap();
        let mut permissions = std::fs::metadata(&target).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&target, permissions).unwrap();
        replace_file(&source, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
    }
}
