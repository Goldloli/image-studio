use std::{
    ffi::c_void,
    io,
    mem::{size_of, zeroed},
    os::windows::{io::AsRawHandle, process::CommandExt},
    process::{Child, Command},
};
#[cfg(test)]
use windows_sys::Win32::System::JobObjects::{
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JobObjectBasicAccountingInformation,
    QueryInformationJobObject,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
        Threading::{CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
    },
};

/// The desktop host's outer lifetime boundary for a native supervisor and all descendants.
pub(crate) struct WindowsProcessJob {
    handle: OwnedHandle,
}

/// Starts a trusted command suspended, joins it to a non-inheritable Job, then resumes it.
pub(crate) fn spawn_suspended_in_job(
    command: &mut Command,
) -> io::Result<(Child, WindowsProcessJob)> {
    let job = WindowsProcessJob::create()?;
    command.creation_flags(CREATE_SUSPENDED);
    let mut child = command.spawn()?;
    let startup = (|| -> io::Result<()> {
        job.assign(&child)?;
        resume_suspended(child.id())
    })();
    if let Err(error) = startup {
        let _ = job.terminate();
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    Ok((child, job))
}

impl WindowsProcessJob {
    fn create() -> io::Result<Self> {
        // SAFETY: null security/name requests an unnamed, non-inheritable Job handle.
        let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        let handle = OwnedHandle::new(raw)?;
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION =
            // SAFETY: all-zero is the documented initial state for this plain C structure.
            unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: the buffer and declared size match JOBOBJECT_EXTENDED_LIMIT_INFORMATION.
        let configured = unsafe {
            SetInformationJobObject(
                handle.0,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast::<c_void>(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { handle })
    }

    fn assign(&self, child: &Child) -> io::Result<()> {
        // SAFETY: both handles are live and the process remains suspended for this call.
        let assigned = unsafe { AssignProcessToJobObject(self.handle.0, child.as_raw_handle()) };
        if assigned == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    fn active_processes(&self) -> io::Result<u32> {
        let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION =
            // SAFETY: all-zero is the documented initial state for this plain C structure.
            unsafe { zeroed() };
        let mut returned = 0u32;
        // SAFETY: the output buffer and declared size match the requested information class.
        let queried = unsafe {
            QueryInformationJobObject(
                self.handle.0,
                JobObjectBasicAccountingInformation,
                (&raw mut accounting).cast::<c_void>(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                &mut returned,
            )
        };
        if queried == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(accounting.ActiveProcesses)
    }

    fn terminate(&self) -> io::Result<()> {
        // SAFETY: the handle is owned by this live host generation boundary.
        if unsafe { TerminateJobObject(self.handle.0, 1) } == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(87) {
                return Ok(());
            }
            Err(error)
        } else {
            Ok(())
        }
    }
}

impl Drop for WindowsProcessJob {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn new(raw: HANDLE) -> io::Result<Self> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(raw))
        }
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns exactly one valid kernel handle.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn resume_suspended(process_id: u32) -> io::Result<()> {
    // SAFETY: the snapshot has no borrowed pointers and is closed by OwnedHandle.
    let snapshot = OwnedHandle::new(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    // SAFETY: entry is writable and dwSize is initialized as required by the API.
    let mut present = unsafe { Thread32First(snapshot.0, &mut entry) } != 0;
    while present {
        if entry.th32OwnerProcessID == process_id {
            // SAFETY: the suspended process still owns this primary thread.
            let thread = OwnedHandle::new(unsafe {
                OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID)
            })?;
            // SAFETY: the handle grants THREAD_SUSPEND_RESUME.
            let previous = unsafe { ResumeThread(thread.0) };
            if previous == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            if previous != 1 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("suspended supervisor had unexpected suspend count {previous}"),
                ));
            }
            return Ok(());
        }
        // SAFETY: snapshot and entry remain valid across enumeration.
        present = unsafe { Thread32Next(snapshot.0, &mut entry) } != 0;
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "suspended supervisor primary thread was not found",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        process::Stdio,
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn closing_the_outer_job_kills_the_live_supervisor() {
        let (mut child, job) = spawn_suspended_in_job(
            Command::new(std::env::var_os("COMSPEC").unwrap())
                .args(["/D", "/C", "ping -n 60 127.0.0.1 >NUL"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null()),
        )
        .unwrap();
        assert!(job.active_processes().unwrap() >= 1);
        let deadline = Instant::now() + Duration::from_secs(5);
        drop(job);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "outer Job close did not terminate its process"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
}
