//! Minimal bounded file log for the desktop host.
//!
//! The GUI process has no console, so diagnostics used to exist only if stderr was redirected.
//! Every event lands in `<data>/logs/app.log` with a fixed line shape, and the file rotates so a
//! long-running install cannot fill the disk.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_BYTES: u64 = 8 * 1024 * 1024;
const KEEP: usize = 3;

struct AppLog {
    path: PathBuf,
    file: Mutex<(File, u64)>,
}

static LOG: OnceLock<AppLog> = OnceLock::new();

impl AppLog {
    fn open(directory: &Path) -> Option<Self> {
        fs::create_dir_all(directory).ok()?;
        let path = directory.join("app.log");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()?;
        let written = file.metadata().map(|meta| meta.len()).unwrap_or(0);
        Some(Self {
            path,
            file: Mutex::new((file, written)),
        })
    }

    fn write(&self, level: &str, event: &str, fields: &[(&str, String)]) {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or(0);
        let mut line = format!("{millis} {level} {event}");
        for (key, value) in fields {
            line.push(' ');
            line.push_str(key);
            line.push('=');
            line.push_str(&value.replace(['\n', '\r'], " "));
        }
        line.push('\n');
        let Ok(mut guard) = self.file.lock() else {
            return;
        };
        let (file, written) = &mut *guard;
        if *written + line.len() as u64 > MAX_BYTES {
            self.rotate(file);
            *written = 0;
        }
        if file.write_all(line.as_bytes()).is_ok() {
            *written += line.len() as u64;
        }
    }

    fn rotate(&self, file: &mut File) {
        let _ = file.flush();
        for index in (1..=KEEP).rev() {
            let from = if index == 1 {
                self.path.clone()
            } else {
                self.path.with_extension(format!("log.{}", index - 1))
            };
            let to = self.path.with_extension(format!("log.{index}"));
            if to.exists() {
                let _ = fs::remove_file(&to);
            }
            if from.exists() {
                let _ = fs::rename(&from, &to);
            }
        }
        if let Ok(reopened) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            *file = reopened;
        }
    }
}

pub fn init(directory: &Path) {
    if let Some(log) = AppLog::open(directory) {
        let _ = LOG.set(log);
    }
}
fn emit(level: &str, event: &str, fields: &[(&str, String)]) {
    if let Some(log) = LOG.get() {
        log.write(level, event, fields);
    }
}

pub fn info(event: &str, fields: &[(&str, String)]) {
    emit("INFO", event, fields);
}

pub fn debug(event: &str, fields: &[(&str, String)]) {
    emit("DEBUG", event, fields);
}

pub fn warn(event: &str, fields: &[(&str, String)]) {
    emit("WARN", event, fields);
}
