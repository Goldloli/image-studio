//! Windows-only acceptance for the per-data-root single instance.
//!
//! Launches the real desktop binary twice. Ignored by default because it opens
//! native windows and needs a desktop session; run it on the Windows machine
//! with:
//!
//! ```text
//! cargo test -p image-studio-desktop --test single_instance -- --ignored --test-threads=1
//! ```
#![cfg(windows)]

use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

fn spawn(root: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_image-studio-desktop"))
        .arg("--data-dir")
        .arg(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn image-studio-desktop")
}

/// The host creates `data/library.sqlite3` and the per-root WebView2 profile
/// before the window is interactive, which is enough to know it owns the root.
fn wait_until_started(root: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if root.join("data").join("library.sqlite3").is_file()
            && root.join("cache").join("webview2").is_dir()
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(error) => panic!("waiting for the second launch failed: {error}"),
        }
    }
    None
}

fn stop(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
#[ignore = "opens native windows; run on a Windows desktop session"]
fn second_launch_on_the_same_root_activates_and_exits_zero() {
    let root = tempfile::tempdir().expect("temp root");
    let mut first = spawn(root.path());
    assert!(
        wait_until_started(root.path(), Duration::from_secs(60)),
        "first instance did not create its data root"
    );

    let started = Instant::now();
    let mut second = spawn(root.path());
    let status = wait_for_exit(&mut second, Duration::from_secs(15))
        .expect("second launch did not exit; it tried to become a second host");
    assert_eq!(
        status.code(),
        Some(0),
        "second launch must exit successfully"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "second launch took {:?}",
        started.elapsed()
    );
    assert!(
        first.try_wait().expect("poll first").is_none(),
        "activating the running instance must not stop it"
    );
    assert!(
        !root
            .path()
            .join("logs")
            .join("startup-failure.log")
            .exists(),
        "an activation is not a startup failure"
    );
    stop(first);
}

#[test]
#[ignore = "opens native windows; run on a Windows desktop session"]
fn different_roots_start_in_parallel() {
    let first_root = tempfile::tempdir().expect("first root");
    let second_root = tempfile::tempdir().expect("second root");
    let mut first = spawn(first_root.path());
    let mut second = spawn(second_root.path());
    assert!(
        wait_until_started(first_root.path(), Duration::from_secs(60)),
        "first root did not start"
    );
    assert!(
        wait_until_started(second_root.path(), Duration::from_secs(60)),
        "second root did not start"
    );
    assert!(
        first.try_wait().expect("poll first").is_none(),
        "a second root must not stop the first instance"
    );
    assert!(
        second.try_wait().expect("poll second").is_none(),
        "the second root stays alive on its own identity"
    );
    stop(first);
    stop(second);
}
