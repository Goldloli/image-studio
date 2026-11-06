pub mod catalog;
pub mod device;
pub mod diagnostics;
pub mod feature_catalog;
pub mod generation;
pub mod installation;
pub mod library;
pub mod logging;
pub mod paths;
pub mod preferences;
pub mod prompt_library;
mod safe_fs;
pub mod storage;
#[cfg(windows)]
pub(crate) mod windows_process;

pub use paths::{AppPaths, PlatformError};

/// Spawn a child without flashing a console window.
///
/// The supervisor, the Python worker and the installer helpers are console programs; a GUI
/// host that spawns them without `CREATE_NO_WINDOW` pops a terminal for every generation.
pub(crate) fn quiet_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut command = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}
