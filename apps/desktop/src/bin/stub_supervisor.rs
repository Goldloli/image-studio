//! Test-only supervisor stub.
//!
//! It ignores the protocol, stdin and any cooperative cancel request, so the host's forced-reap
//! path can be exercised: the generation controller must terminate the process group once the
//! cancel grace deadline passes instead of waiting for the child to exit on its own.

use std::{thread, time::Duration};

fn main() {
    // A marker proves the host really spawned this process during the test.
    if let Ok(path) = std::env::var("STUB_SUPERVISOR_MARKER") {
        let _ = std::fs::write(path, b"started");
    }
    // Long enough that the host has to kill it, short enough that a leaked process exits by itself.
    thread::sleep(Duration::from_secs(120));
}
