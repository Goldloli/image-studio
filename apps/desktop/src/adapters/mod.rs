pub mod catalog;
pub mod diagnostics;
pub mod feature_catalog;
pub mod generation;
pub mod installation;
pub mod library;
pub mod paths;
pub mod preferences;
pub mod prompt_library;
mod safe_fs;
pub mod storage;

pub use paths::{AppPaths, PlatformError};
