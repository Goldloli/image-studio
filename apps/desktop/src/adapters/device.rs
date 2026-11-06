//! Best-effort local device facts used to gate install/execution entry points.
//!
//! The probe is intentionally bounded and failure-tolerant: a missing `nvidia-smi`, a hung
//! driver query or an unavailable Win32 call must degrade to "unknown", never panic or block the
//! host. Hardware gates are advisory facts for the model catalog; an unknown fact is treated as
//! "not proven", so a model requiring a GPU/amount of RAM is disabled with an actionable reason
//! instead of silently starting and failing later.
#[cfg(windows)]
use std::time::{Duration, Instant};

/// The probe timeout for the NVIDIA driver query. `nvidia-smi` answers in well under a second on
/// a healthy target machine; a driver hang must not stall the host.
#[cfg(windows)]
const NVIDIA_SMI_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(windows)]
const MIB_PER_GIB: u64 = 1024;

/// Local device facts. `vram_gb` is `None` when no NVIDIA GPU query is available (for example a
/// macOS host or a machine without `nvidia-smi`); `ram_gb` is `0` when the total physical memory
/// could not be read.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeviceFacts {
    pub vram_gb: Option<u32>,
    pub ram_gb: u32,
    /// NVIDIA compute capability as `(major, minor)`, e.g. `(7, 5)` for Turing. `None` when the
    /// driver does not report it or no NVIDIA GPU is present; an unknown capability is not treated
    /// as a hard failure because older drivers omit the field while still reporting the VRAM.
    pub compute_capability: Option<(u32, u32)>,
}

impl DeviceFacts {
    /// Whether this device can execute the compact tier on a Turing-or-newer GPU. Unknown compute
    /// capability passes (`None`) so a missing driver field does not silently disable the tier; the
    /// worker probe still has to prove fp16 execution before a generation runs.
    pub fn meets_compact_compute_capability(&self) -> bool {
        self.compute_capability
            .is_none_or(|(major, minor)| (major, minor) >= (7, 5))
    }
}

/// The outcome of comparing [`DeviceFacts`] with a registered model's minimum requirements.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HardwareGate {
    pub eligible: bool,
    /// Actionable Chinese explanation when `eligible` is false; `None` when the device satisfies
    /// (or is not required to prove) every threshold.
    pub reason: Option<String>,
}

impl HardwareGate {
    pub fn eligible() -> Self {
        Self {
            eligible: true,
            reason: None,
        }
    }

    pub fn blocked(reason: String) -> Self {
        Self {
            eligible: false,
            reason: Some(reason),
        }
    }
}

impl DeviceFacts {
    /// Returns the process-wide cached facts. The first call probes; later calls are free. Device
    /// capabilities do not change while the host is running, so no refresh loop is needed.
    pub fn cached() -> Self {
        use std::sync::OnceLock;
        static FACTS: OnceLock<DeviceFacts> = OnceLock::new();
        *FACTS.get_or_init(Self::detect)
    }

    /// Probes the local device without caching. Best effort, bounded, never panics.
    pub fn detect() -> Self {
        // The platform stub returns (None, None) off Windows, so the call can stay uniform and
        // the stub is never dead code under `clippy -D warnings`.
        let (vram_gb, compute_capability) = detect_gpu_facts();
        Self {
            vram_gb,
            ram_gb: detect_ram_gb(),
            compute_capability,
        }
    }

    /// Compares these facts with the model thresholds from the catalog. A `None` or `0` threshold
    /// means the model does not require that resource. A required fact that is unknown is treated
    /// as unsatisfied so the entry point is disabled with a reason instead of failing later.
    pub fn gate(&self, min_vram_gb: Option<u32>, min_ram_gb: Option<u32>) -> HardwareGate {
        let mut missing = Vec::new();
        let mut unconfirmed = Vec::new();
        if let Some(min) = min_vram_gb.filter(|value| *value > 0) {
            match self.vram_gb {
                Some(actual) if actual < min => {
                    missing.push(format!("显存 ≥ {min} GB（当前 {actual} GB）"));
                }
                Some(_) => {}
                None => unconfirmed.push(format!("显存 ≥ {min} GB（当前无法读取）")),
            }
        }
        if let Some(min) = min_ram_gb.filter(|value| *value > 0) {
            if self.ram_gb == 0 {
                unconfirmed.push(format!("内存 ≥ {min} GB（当前无法读取）"));
            } else if self.ram_gb < min {
                missing.push(format!("内存 ≥ {min} GB（当前 {} GB）", self.ram_gb));
            }
        }
        if missing.is_empty() && unconfirmed.is_empty() {
            return HardwareGate::eligible();
        }
        let mut problems = Vec::new();
        if !missing.is_empty() {
            problems.push(format!("当前设备不满足硬件门槛：{}", missing.join("、")));
        }
        if !unconfirmed.is_empty() {
            problems.push(format!(
                "无法确认硬件门槛：{}；请检查 NVIDIA 驱动与 nvidia-smi 是否可用",
                unconfirmed.join("、")
            ));
        }
        HardwareGate::blocked(format!(
            "{}。为避免安装或生成中途失败，安装与生成入口已禁用。",
            problems.join("；")
        ))
    }

    /// Human-readable facts for the settings page. Unknown fields are stated as unknown instead of
    /// being rendered as zero.
    pub fn label(&self) -> String {
        let vram = match self.vram_gb {
            Some(value) => format!("显存 {value} GB"),
            None => "显存未知".into(),
        };
        let ram = if self.ram_gb == 0 {
            "内存未知".into()
        } else {
            format!("内存 {} GB", self.ram_gb)
        };
        format!("{vram} · {ram}")
    }
}

#[cfg(windows)]
fn mib_to_gib(mib: u64) -> u32 {
    // Round to the nearest GiB so that e.g. 12282 MiB is reported as 12 GB instead of 11 GB.
    ((mib + MIB_PER_GIB / 2) / MIB_PER_GIB) as u32
}

/// Parses the first non-empty line of `nvidia-smi --query-gpu=memory.total,compute_cap
/// --format=csv,noheader,nounits`. The first column is MiB, the second optional column is the
/// `major.minor` compute capability. A single-column answer (older driver) keeps the VRAM and
/// reports an unknown capability instead of failing the whole probe.
#[cfg(windows)]
fn parse_nvidia_smi_gpu(output: &str) -> (Option<u64>, Option<(u32, u32)>) {
    let Some(line) = output.lines().map(str::trim).find(|line| !line.is_empty()) else {
        return (None, None);
    };
    let mut columns = line.split(',');
    let mib = columns
        .next()
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0);
    let compute = columns
        .next()
        .map(str::trim)
        .and_then(parse_compute_capability);
    (mib, compute)
}

#[cfg(windows)]
fn parse_compute_capability(text: &str) -> Option<(u32, u32)> {
    let (major, minor) = text.split_once('.')?;
    let major = major.trim().parse::<u32>().ok()?;
    let minor = minor.trim().parse::<u32>().ok()?;
    ((major, minor) >= (5, 0) && (major, minor) <= (20, 0)).then_some((major, minor))
}

/// Times a bounded `nvidia-smi` query and returns its stdout. Any failure, timeout or non-zero
/// exit yields `None`; the probe must never block the host.
#[cfg(windows)]
fn nvidia_smi(args: &[&str]) -> Option<String> {
    use std::{
        io::Read,
        os::windows::process::CommandExt,
        process::{Command, Stdio},
    };
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut child = Command::new("nvidia-smi")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .ok()?;
    let deadline = Instant::now() + NVIDIA_SMI_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut output = String::new();
    child.stdout.take()?.read_to_string(&mut output).ok()?;
    Some(output)
}

#[cfg(windows)]
fn detect_gpu_facts() -> (Option<u32>, Option<(u32, u32)>) {
    let Some(output) = nvidia_smi(&[
        "--query-gpu=memory.total,compute_cap",
        "--format=csv,noheader,nounits",
    ]) else {
        return (None, None);
    };
    let (mib, compute) = parse_nvidia_smi_gpu(&output);
    (mib.map(mib_to_gib), compute)
}

#[cfg(not(windows))]
fn detect_gpu_facts() -> (Option<u32>, Option<(u32, u32)>) {
    (None, None)
}

#[cfg(windows)]
fn detect_ram_gb() -> u32 {
    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }

    // kernel32 is linked by the Windows standard library; this declaration only adds the symbol.
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }

    let mut status: MemoryStatusEx = {
        // SAFETY: this plain C structure is valid when zeroed; `length` is set below.
        let mut status = unsafe { std::mem::zeroed::<MemoryStatusEx>() };
        status.length = std::mem::size_of::<MemoryStatusEx>() as u32;
        status
    };
    // SAFETY: `status` is a live, correctly sized MEMORYSTATUSEX buffer.
    let succeeded = unsafe { GlobalMemoryStatusEx(&mut status) };
    if succeeded == 0 {
        0
    } else {
        bytes_to_gib_rounded(status.total_phys)
    }
}

/// Rounds total physical memory to the nearest GiB. A machine sold as 32 GB reports slightly less
/// than 32 GiB total (firmware-reserved memory, e.g. 31.94 GiB), so flooring would wrongly mark the
/// certified 32 GB target machine as below its registered threshold.
#[cfg(windows)]
fn bytes_to_gib_rounded(bytes: u64) -> u32 {
    const GIB: u64 = 1024 * 1024 * 1024;
    ((bytes + GIB / 2) / GIB) as u32
}

#[cfg(not(windows))]
fn detect_ram_gb() -> u32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(vram_gb: Option<u32>, ram_gb: u32) -> DeviceFacts {
        DeviceFacts {
            vram_gb,
            ram_gb,
            compute_capability: None,
        }
    }

    fn fake_with_capability(vram_gb: u32, ram_gb: u32, capability: (u32, u32)) -> DeviceFacts {
        DeviceFacts {
            vram_gb: Some(vram_gb),
            ram_gb,
            compute_capability: Some(capability),
        }
    }

    #[test]
    #[cfg(windows)]
    fn nvidia_smi_memory_total_and_compute_capability_are_parsed_from_the_first_gpu() {
        let (mib, compute) = parse_nvidia_smi_gpu("12288, 8.6\n");
        assert_eq!(mib, Some(12288));
        assert_eq!(compute, Some((8, 6)));
        // A Turing card reports 7.5.
        assert_eq!(parse_nvidia_smi_gpu("8192, 7.5\n").1, Some((7, 5)));
        // Older drivers only answer the memory column; the capability stays unknown.
        assert_eq!(parse_nvidia_smi_gpu("12288\n"), (Some(12288), None));
        assert_eq!(parse_nvidia_smi_gpu("0\n"), (None, None));
        assert_eq!(parse_nvidia_smi_gpu("N/A\n"), (None, None));
        assert_eq!(parse_nvidia_smi_gpu("\n\n"), (None, None));
        assert_eq!(parse_nvidia_smi_gpu("8192, N/A\n").1, None);
    }

    #[test]
    #[cfg(windows)]
    fn compute_capability_parser_rejects_implausible_values() {
        assert_eq!(parse_compute_capability("7.5"), Some((7, 5)));
        assert_eq!(parse_compute_capability("12.0"), Some((12, 0)));
        assert_eq!(parse_compute_capability("N/A"), None);
        assert_eq!(parse_compute_capability("nonsense"), None);
        assert_eq!(parse_compute_capability("1.0"), None);
    }

    #[test]
    fn compact_tier_requires_turing_or_newer_only_when_the_capability_is_known() {
        assert!(fake(None, 32).meets_compact_compute_capability());
        assert!(fake_with_capability(8, 32, (7, 5)).meets_compact_compute_capability());
        assert!(fake_with_capability(8, 32, (8, 6)).meets_compact_compute_capability());
        assert!(!fake_with_capability(8, 32, (6, 1)).meets_compact_compute_capability());
    }

    #[test]
    #[cfg(windows)]
    fn memory_total_mib_is_rounded_to_nearest_gib() {
        assert_eq!(mib_to_gib(12288), 12);
        assert_eq!(mib_to_gib(12282), 12);
        assert_eq!(mib_to_gib(8192), 8);
        assert_eq!(mib_to_gib(7168), 7);
        assert_eq!(mib_to_gib(0), 0);
    }

    #[test]
    #[cfg(windows)]
    fn total_physical_memory_bytes_are_rounded_to_the_registered_gigabytes() {
        assert_eq!(bytes_to_gib_rounded(32 * 1024 * 1024 * 1024), 32);
        // A 32 GB machine loses a little total memory to firmware; it must still pass a 32 GB
        // threshold instead of being reported as 31 GB.
        assert_eq!(bytes_to_gib_rounded(34_293_030_912), 32);
        assert_eq!(bytes_to_gib_rounded(16 * 1024 * 1024 * 1024), 16);
        assert_eq!(bytes_to_gib_rounded(0), 0);
    }

    #[test]
    fn models_without_thresholds_are_eligible_on_any_device() {
        let gate = fake(None, 0).gate(None, None);
        assert_eq!(gate, HardwareGate::eligible());
        let gate = fake(None, 0).gate(Some(0), Some(0));
        assert_eq!(gate, HardwareGate::eligible());
    }

    #[test]
    fn satisfied_thresholds_are_eligible() {
        let device = fake(Some(12), 32);
        assert_eq!(device.gate(Some(12), Some(32)), HardwareGate::eligible());
        assert_eq!(device.gate(Some(4), Some(8)), HardwareGate::eligible());
        // A CPU-only route accepts a device with no readable VRAM.
        assert_eq!(
            fake(None, 16).gate(Some(0), Some(16)),
            HardwareGate::eligible()
        );
    }

    #[test]
    fn unsatisfied_thresholds_are_disabled_with_an_actionable_reason() {
        let gate = fake(Some(8), 16).gate(Some(12), Some(32));
        assert!(!gate.eligible);
        let reason = gate.reason.unwrap();
        assert!(reason.contains("显存 ≥ 12 GB（当前 8 GB）"));
        assert!(reason.contains("内存 ≥ 32 GB（当前 16 GB）"));
        assert!(reason.contains("安装与生成入口已禁用"));
    }

    #[test]
    fn unknown_device_facts_disable_required_thresholds_instead_of_passing() {
        let gate = fake(None, 0).gate(Some(12), Some(32));
        assert!(!gate.eligible);
        let reason = gate.reason.unwrap();
        assert!(reason.contains("当前无法读取"));
        assert!(reason.contains("nvidia-smi"));
        // The CPU-only BiRefNet route stays available when RAM is known to be sufficient.
        assert_eq!(
            fake(None, 16).gate(Some(0), Some(16)),
            HardwareGate::eligible()
        );
        assert!(!fake(None, 16).gate(Some(4), Some(8)).eligible);
    }

    #[test]
    fn label_does_not_render_unknown_facts_as_zero() {
        assert_eq!(fake(Some(12), 32).label(), "显存 12 GB · 内存 32 GB");
        assert_eq!(fake(None, 0).label(), "显存未知 · 内存未知");
        assert_eq!(fake(Some(12), 0).label(), "显存 12 GB · 内存未知");
    }

    #[cfg(windows)]
    #[test]
    fn live_probe_reports_total_memory_without_panicking() {
        let facts = DeviceFacts::detect();
        assert!(
            facts.ram_gb > 0,
            "GlobalMemoryStatusEx must report total physical memory"
        );
        if let Some(vram) = facts.vram_gb {
            assert!(
                vram > 0,
                "nvidia-smi must not report a zero total when present"
            );
        }
    }
}
