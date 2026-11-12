//! Per-data-root single instance and window activation.
//!
//! A second launch against a data root that is already open used to build a
//! second host and collide on the WebView2 user data folder; the user saw a
//! startup-failure dialog (or, before the diagnostics fix, a bare 101). This
//! module hands the second launch back to the window that already owns the
//! root: session-local named objects identify the owner, an activation event
//! asks it to raise its window, and an acknowledgement makes the outcome
//! deterministic.
//!
//! The identity is the canonical data path, so independent `--data-dir` roots
//! keep running in parallel. Windows only: other platforms always claim the
//! primary role, which keeps the platform build and its tests unchanged.

use std::path::Path;
use std::time::Duration;

/// How long the second launch waits for the running instance to acknowledge
/// the activation. A cold main window can take a few seconds to appear and the
/// request is queued on an auto-reset event, so the bound covers startup rather
/// than only the warm case.
pub const ACTIVATION_ACK_TIMEOUT: Duration = Duration::from_secs(5);

/// Result of claiming a data root at process start.
pub enum Claim {
    /// This process owns the root and must serve activation requests.
    Primary(Primary),
    /// Another process owns the root; ask it to activate and exit.
    Secondary(Secondary),
}

/// Outcome reported to the second launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Activation {
    /// The running instance acknowledged; its window was raised.
    Activated,
    /// The instance exists but did not acknowledge within the timeout.
    TimedOut,
    /// The owner's activation objects are not reachable (for example another
    /// Windows login session owns the root).
    Unavailable,
}

#[cfg(windows)]
mod platform {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::Arc;
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
    use windows_sys::Win32::System::Threading::{
        CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, OpenEventW, SetEvent, WaitForSingleObject,
    };

    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_TIMEOUT: u32 = 258;
    /// WaitForSingleObject requires this access bit; windows-sys 0.61 does not
    /// export the generic constant, only the per-object access rights.
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const INFINITE: u32 = u32::MAX;
    const EVENT_OPEN_ATTEMPTS: u32 = 20;
    const EVENT_OPEN_INTERVAL: Duration = Duration::from_millis(50);

    /// Owns one kernel handle and closes it on drop.
    struct Handle(isize);

    // Kernel handles are process-wide references; sharing them across threads
    // is the intended use of `WaitForSingleObject`/`SetEvent`.
    unsafe impl Send for Handle {}
    unsafe impl Sync for Handle {}

    impl Handle {
        fn from_raw(raw: HANDLE) -> Option<Self> {
            (!raw.is_null()).then_some(Self(raw as isize))
        }

        fn raw(&self) -> HANDLE {
            self.0 as HANDLE
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: the handle was created or opened by this process and is
            // closed exactly once.
            unsafe { CloseHandle(self.raw()) };
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain(Some(0)).collect()
    }

    /// Stable 16 hex digit identity of the canonical data root.
    ///
    /// Both processes of the same build must derive the same identity from the
    /// same root, including a trailing separator or a different letter case
    /// (Windows paths are case-insensitive).
    fn root_identity(data_root: &Path) -> String {
        let canonical = data_root
            .canonicalize()
            .unwrap_or_else(|_| data_root.to_path_buf());
        let mut normalized = canonical.to_string_lossy().replace('\\', "/");
        if normalized.ends_with('/') {
            normalized.pop();
        }
        let digest = Sha256::digest(normalized.to_lowercase().as_bytes());
        digest
            .iter()
            .take(8)
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn object_name(identity: &str, suffix: &str) -> Vec<u16> {
        wide(&format!("Local\\image-studio-{identity}{suffix}"))
    }

    /// Session-local ownership: the mutex must live as long as the process, so
    /// the primary guard keeps it (when the API cooperates).
    pub struct PrimaryOwner {
        _mutex: Option<Handle>,
        events: Option<(Arc<Handle>, Arc<Handle>)>,
    }

    impl PrimaryOwner {
        fn new(mutex: Option<Handle>, identity: &str) -> Self {
            let activate = create_event(&object_name(identity, "-activate")).map(Arc::new);
            let ack = create_event(&object_name(identity, "-ack")).map(Arc::new);
            let events = match (activate, ack) {
                (Some(activate), Some(ack)) => Some((activate, ack)),
                _ => None,
            };
            Self {
                _mutex: mutex,
                events,
            }
        }

        pub fn receiver(&self) -> ActivationReceiver {
            ActivationReceiver {
                activate: self.events.as_ref().map(|(activate, _)| activate.clone()),
                ack: self.events.as_ref().map(|(_, ack)| ack.clone()),
            }
        }
    }

    fn create_event(name: &[u16]) -> Option<Handle> {
        // Auto-reset, initially non-signalled: a signal sent before the listener
        // starts stays pending until the first wait consumes it.
        let raw = unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) };
        Handle::from_raw(raw)
    }

    fn open_event(name: &[u16]) -> Option<Handle> {
        // Waiting needs SYNCHRONIZE; EVENT_MODIFY_STATE alone only allows
        // SetEvent and would make WaitForSingleObject fail instead of waiting.
        let raw = unsafe { OpenEventW(EVENT_MODIFY_STATE | SYNCHRONIZE, 0, name.as_ptr()) };
        Handle::from_raw(raw)
    }

    pub struct ActivationReceiver {
        activate: Option<Arc<Handle>>,
        ack: Option<Arc<Handle>>,
    }

    impl ActivationReceiver {
        /// Blocks until the owner is asked to activate its window. Returns
        /// `false` when the objects are unavailable so the listener can stop.
        pub fn wait(&self) -> bool {
            let Some(activate) = &self.activate else {
                return false;
            };
            unsafe { WaitForSingleObject(activate.raw(), INFINITE) == WAIT_OBJECT_0 }
        }

        pub fn acknowledge(&self) {
            if let Some(ack) = &self.ack {
                unsafe { SetEvent(ack.raw()) };
            }
        }
    }

    pub struct Secondary {
        identity: String,
    }

    impl Secondary {
        pub fn request_activation(&self) -> Activation {
            let activate_name = object_name(&self.identity, "-activate");
            let ack_name = object_name(&self.identity, "-ack");
            let mut opened = None;
            for _ in 0..EVENT_OPEN_ATTEMPTS {
                if let (Some(activate), Some(ack)) =
                    (open_event(&activate_name), open_event(&ack_name))
                {
                    opened = Some((activate, ack));
                    break;
                }
                std::thread::sleep(EVENT_OPEN_INTERVAL);
            }
            let Some((activate, ack)) = opened else {
                return Activation::Unavailable;
            };
            unsafe { SetEvent(activate.raw()) };
            let wait = unsafe {
                WaitForSingleObject(
                    ack.raw(),
                    ACTIVATION_ACK_TIMEOUT.as_millis().min(u128::from(u32::MAX)) as u32,
                )
            };
            match wait {
                WAIT_OBJECT_0 => Activation::Activated,
                WAIT_TIMEOUT => Activation::TimedOut,
                _ => Activation::Unavailable,
            }
        }
    }

    pub fn claim(data_root: &Path) -> Claim {
        let identity = root_identity(data_root);
        let name = object_name(&identity, "");
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        let Some(handle) = Handle::from_raw(handle) else {
            // Losing the identity is not worth blocking startup: continue as a
            // plain primary without activation support.
            return Claim::Primary(PrimaryOwner::new(None, &identity));
        };
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            return Claim::Secondary(Secondary { identity });
        }
        Claim::Primary(PrimaryOwner::new(Some(handle), &identity))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::path::PathBuf;

        #[test]
        fn identity_is_stable_across_spellings() {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().to_path_buf();
            let mut trailing = path.clone().into_os_string();
            trailing.push("\\");
            let upper = PathBuf::from(path.to_string_lossy().to_uppercase());
            assert_eq!(
                root_identity(&path),
                root_identity(&PathBuf::from(trailing))
            );
            assert_eq!(root_identity(&path), root_identity(&upper));
        }

        #[test]
        fn identity_differs_per_root() {
            let first = tempfile::tempdir().unwrap();
            let second = tempfile::tempdir().unwrap();
            assert_ne!(root_identity(first.path()), root_identity(second.path()));
        }

        #[test]
        fn a_second_claim_in_the_same_process_is_secondary() {
            let root = tempfile::tempdir().unwrap();
            let first = claim(root.path());
            assert!(matches!(first, Claim::Primary(_)));
            let second = claim(root.path());
            assert!(matches!(second, Claim::Secondary(_)));
            drop(second);
            drop(first);
            // The objects vanish with the last handle, so a later launch owns the root again.
            let third = claim(root.path());
            assert!(matches!(third, Claim::Primary(_)));
        }

        #[test]
        fn different_roots_are_both_primary() {
            let first_root = tempfile::tempdir().unwrap();
            let second_root = tempfile::tempdir().unwrap();
            let first = claim(first_root.path());
            let second = claim(second_root.path());
            assert!(matches!(first, Claim::Primary(_)));
            assert!(matches!(second, Claim::Primary(_)));
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub struct PrimaryOwner;

    impl PrimaryOwner {
        pub fn receiver(&self) -> ActivationReceiver {
            ActivationReceiver
        }
    }

    pub struct ActivationReceiver;

    impl ActivationReceiver {
        pub fn wait(&self) -> bool {
            false
        }

        pub fn acknowledge(&self) {}
    }

    pub struct Secondary;

    impl Secondary {
        pub fn request_activation(&self) -> Activation {
            Activation::Unavailable
        }
    }

    pub fn claim(_data_root: &Path) -> Claim {
        Claim::Primary(PrimaryOwner)
    }
}

pub use platform::{ActivationReceiver, PrimaryOwner as Primary, Secondary, claim};
