//! Tauri composition root. Domain contracts and diagnostic supervision remain Rust-owned.
pub mod adapters;
pub mod host;
pub mod options;
pub mod studio;

pub const DIAGNOSTIC_MODES: &[&str] = &[
    "normal",
    "bad-version",
    "malformed",
    "oversize",
    "duplicate",
    "wrong-session",
    "wrong-request",
    "crash",
    "hang",
    "ignore-shutdown",
    "flood",
    "descendant",
];
