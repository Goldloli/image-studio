//! Startup failure reporting and per-data-root system-WebView profile isolation.
//!
//! Tauri panics while creating the native window when the platform WebView
//! runtime is missing, broken or cannot use its user data folder. A release
//! build may hide the console, so without this module the only observable
//! result is exit code 101 and no explanation. The hook below turns that class
//! of failure into a log record, a native message box where supported, and the
//! documented exit code.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Documented exit code for a desktop shell that cannot create its WebView2 window.
pub const STARTUP_FAILURE_EXIT_CODE: i32 = 3;

/// File written under `<data root>/logs` when the shell cannot start.
pub const STARTUP_FAILURE_LOG: &str = "startup-failure.log";

/// Tauri prefixes its `setup` panic with this text (`tauri::app::App::run`).
const SETUP_FAILURE_PREFIX: &str = "Failed to setup app";

/// WebView2 reads this variable when the API is called with an empty folder.
#[cfg(windows)]
const WEBVIEW2_PROFILE_VARIABLE: &str = "WEBVIEW2_USER_DATA_FOLDER";

/// Directory holding this data root's WebView2 profile.
///
/// The WebView2 default (`%LOCALAPPDATA%\<identifier>\EBWebView`) is shared by
/// every process of the application, including instances with a different
/// `--data-dir`. Keeping the profile inside the data root makes independent
/// roots structurally independent and keep their browser state disposable.
pub fn webview_profile_directory(cache: &Path) -> PathBuf {
    cache.join("webview2")
}

/// Creates the profile directory and points WebView2 at it.
pub fn configure_webview_profile(cache: &Path) -> std::io::Result<PathBuf> {
    let directory = prepare_webview_profile(cache)?;
    #[cfg(windows)]
    {
        // SAFETY: called from the main thread before Tauri creates the webview
        // or spawns worker threads; no other thread reads the environment yet.
        unsafe { std::env::set_var(WEBVIEW2_PROFILE_VARIABLE, &directory) };
    }
    Ok(directory)
}

/// Creates the profile directory without touching the process environment.
fn prepare_webview_profile(cache: &Path) -> std::io::Result<PathBuf> {
    let directory = webview_profile_directory(cache);
    std::fs::create_dir_all(&directory)?;
    Ok(directory)
}

/// Installs a panic hook that reports Tauri setup failures instead of letting
/// them disappear as a bare 101 exit code. Unrelated panics keep the default
/// behaviour, so existing crash reporting is unchanged.
pub fn install_panic_hook(logs: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let Some(detail) = startup_failure_detail(info.payload()) else {
            previous(info);
            return;
        };
        fail_startup(&logs, &detail);
    }));
}

/// Writes the failure record, shows it to the user and exits with
/// [`STARTUP_FAILURE_EXIT_CODE`].
pub fn fail_startup(logs: &Path, detail: &str) -> ! {
    let report = failure_report(logs, detail, SystemTime::now());
    let _ = std::fs::write(logs.join(STARTUP_FAILURE_LOG), &report);
    show_failure_dialog(&report);
    std::process::exit(STARTUP_FAILURE_EXIT_CODE)
}

/// Returns the panic message when it is a Tauri setup failure.
fn startup_failure_detail(payload: &(dyn std::any::Any + Send)) -> Option<String> {
    let message = payload
        .downcast_ref::<&str>()
        .map(|value| (*value).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())?;
    message.starts_with(SETUP_FAILURE_PREFIX).then_some(message)
}

/// Human readable report written next to the application logs.
fn failure_report(logs: &Path, detail: &str, now: SystemTime) -> String {
    let seconds = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let log_path = logs.join(STARTUP_FAILURE_LOG);
    #[cfg(windows)]
    let advice = "确认 Microsoft Edge WebView2 Runtime 已安装且可启动；确认数据根可写；仍失败时保留本文件供排查。";
    #[cfg(not(windows))]
    let advice = "确认系统 WebView 可启动；确认数据根可写；仍失败时保留本文件供排查。";
    format!(
        "Image Studio 启动失败\n时间（UTC）：{}\n原因：{}\n日志：{}\n建议：{advice}\n",
        format_utc_timestamp(seconds),
        detail.trim(),
        log_path.display()
    )
}

/// `1970-01-01T00:00:00Z` style UTC timestamp without extra dependencies.
fn format_utc_timestamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        (rest % 3_600) / 60,
        rest % 60
    )
}

/// Gregorian date for a day count since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_position + 2) / 5 + 1) as u32;
    let month = if month_position < 10 {
        month_position + 3
    } else {
        month_position - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(windows)]
fn show_failure_dialog(report: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW,
    };
    let caption: Vec<u16> = "Image Studio 启动失败\0".encode_utf16().collect();
    let body: Vec<u16> = format!("{report}\0").encode_utf16().collect();
    // SAFETY: both strings are NUL terminated and outlive the call; a null owner
    // window is valid for a standalone message box.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            body.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }
}

/// Other platforms keep the previous behaviour: the log is written by the
/// caller and the panic message is already visible on stderr.
#[cfg(not(windows))]
fn show_failure_dialog(_report: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tauri_setup_panics_are_reported_and_other_panics_are_not() {
        let setup: &(dyn std::any::Any + Send) =
            &"Failed to setup app: runtime error: Could not find the webview runtime";
        assert!(startup_failure_detail(setup).is_some());
        let owned: &(dyn std::any::Any + Send) =
            &String::from("Failed to setup app: runtime error: locked profile");
        assert!(startup_failure_detail(owned).is_some());
        let unrelated: &(dyn std::any::Any + Send) = &"index out of bounds";
        assert!(startup_failure_detail(unrelated).is_none());
    }

    #[test]
    fn failure_report_names_the_reason_and_the_log_path() {
        let root = tempfile::tempdir().unwrap();
        let logs = root.path().join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        let report = failure_report(
            &logs,
            "Failed to setup app: runtime error: missing webview runtime",
            UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
        );
        assert!(report.contains("启动失败"));
        assert!(report.contains("2023-11-14T22:13:20Z"));
        assert!(report.contains("Failed to setup app: runtime error: missing webview runtime"));
        assert!(report.contains(&logs.join(STARTUP_FAILURE_LOG).display().to_string()));
        std::fs::write(logs.join(STARTUP_FAILURE_LOG), report).unwrap();
        assert!(logs.join(STARTUP_FAILURE_LOG).is_file());
    }

    #[test]
    fn utc_timestamps_are_fixed_width() {
        assert_eq!(format_utc_timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc_timestamp(946_684_800), "2000-01-01T00:00:00Z");
        assert_eq!(format_utc_timestamp(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(format_utc_timestamp(1_793_708_400), "2026-11-03T12:20:00Z");
    }

    #[test]
    fn webview_profile_is_owned_by_the_data_root() {
        let cache = Path::new("C:/data/cache");
        assert_eq!(
            webview_profile_directory(cache),
            PathBuf::from("C:/data/cache/webview2")
        );
        let root = tempfile::tempdir().unwrap();
        let created = prepare_webview_profile(root.path()).unwrap();
        assert!(created.is_dir());
        assert!(created.ends_with("webview2"));
    }
}
