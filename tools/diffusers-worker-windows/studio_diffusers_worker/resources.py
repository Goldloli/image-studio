"""Resource sampling and Gate 0 budget enforcement for the worker process.

The Windows target budget (design.md sections 1 and 10) is: load <= 600 s,
single generation <= 900 s, CUDA reserved <= 10.5 GiB without WDDM shared-memory
fallback, process-tree private commit <= 24 GiB, and system available RAM never
below 2 GiB. Fixed identities with a `--probe`/`--prepare` phase run the same
budget checks and fail closed instead of oversubscribing the machine.
"""

from __future__ import annotations

import ctypes
import threading
import time
from ctypes import wintypes

LOAD_LIMIT_SECONDS = 600.0
GENERATION_LIMIT_SECONDS = 900.0
CUDA_RESERVED_LIMIT_GIB = 10.5
PRIVATE_COMMIT_LIMIT_GIB = 24.0
AVAILABLE_RAM_MINIMUM_GIB = 2.0


class PROCESS_MEMORY_COUNTERS_EX(ctypes.Structure):
    _fields_ = [
        ("cb", wintypes.DWORD),
        ("PageFaultCount", wintypes.DWORD),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
        ("PrivateUsage", ctypes.c_size_t),
    ]


class MEMORYSTATUSEX(ctypes.Structure):
    _fields_ = [
        ("dwLength", wintypes.DWORD),
        ("dwMemoryLoad", wintypes.DWORD),
        ("ullTotalPhys", ctypes.c_ulonglong),
        ("ullAvailPhys", ctypes.c_ulonglong),
        ("ullTotalPageFile", ctypes.c_ulonglong),
        ("ullAvailPageFile", ctypes.c_ulonglong),
        ("ullTotalVirtual", ctypes.c_ulonglong),
        ("ullAvailVirtual", ctypes.c_ulonglong),
        ("ullAvailExtendedVirtual", ctypes.c_ulonglong),
    ]


_KERNEL32 = ctypes.windll.kernel32
_KERNEL32.GetCurrentProcess.restype = wintypes.HANDLE
_KERNEL32.K32GetProcessMemoryInfo.argtypes = [
    wintypes.HANDLE,
    ctypes.POINTER(PROCESS_MEMORY_COUNTERS_EX),
    wintypes.DWORD,
]
_KERNEL32.K32GetProcessMemoryInfo.restype = wintypes.BOOL


def _memory_counters() -> PROCESS_MEMORY_COUNTERS_EX:
    counters = PROCESS_MEMORY_COUNTERS_EX()
    counters.cb = ctypes.sizeof(PROCESS_MEMORY_COUNTERS_EX)
    if not _KERNEL32.K32GetProcessMemoryInfo(
        _KERNEL32.GetCurrentProcess(),
        ctypes.byref(counters),
        counters.cb,
    ):
        raise ctypes.WinError()
    return counters


def process_private_bytes() -> int:
    return int(_memory_counters().PrivateUsage)


def process_commit_peak_bytes() -> int:
    return int(_memory_counters().PeakPagefileUsage)


def system_memory() -> dict[str, int]:
    status = MEMORYSTATUSEX()
    status.dwLength = ctypes.sizeof(MEMORYSTATUSEX)
    if not ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status)):
        raise ctypes.WinError()
    return {
        "physical_total_bytes": int(status.ullTotalPhys),
        "physical_available_bytes": int(status.ullAvailPhys),
        "commit_limit_bytes": int(status.ullTotalPageFile),
        "commit_available_bytes": int(status.ullAvailPageFile),
    }


class ResourceSampler:
    """Sample own process private commit and system available RAM."""

    def __init__(self, interval_seconds: float = 0.2) -> None:
        self.interval = interval_seconds
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self.private_peak = 0
        self.available_ram_min = 1 << 62
        self.system_total = 0
        self.samples = 0

    def _sample(self) -> None:
        self.private_peak = max(self.private_peak, process_private_bytes())
        memory = system_memory()
        self.system_total = memory["physical_total_bytes"]
        self.available_ram_min = min(
            self.available_ram_min, memory["physical_available_bytes"]
        )
        self.samples += 1

    def _loop(self) -> None:
        while not self._stop.wait(self.interval):
            try:
                self._sample()
            except Exception:  # noqa: BLE001 - sampling is best effort
                pass

    def reset(self) -> None:
        self._stop = threading.Event()
        self.private_peak = 0
        self.available_ram_min = 1 << 62
        self.system_total = 0
        self.samples = 0

    def start(self) -> None:
        self._stop = threading.Event()
        self._sample()
        self._thread = threading.Thread(target=self._loop, daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        if self._thread is not None:
            self._thread.join(timeout=10)

    def snapshot(self) -> dict:
        return {
            "process_private_commit_peak_gib": round(self.private_peak / 1024**3, 3),
            "system_available_ram_min_gib": round(self.available_ram_min / 1024**3, 3),
            "system_physical_total_gib": round(self.system_total / 1024**3, 3),
            "sample_interval_seconds": self.interval,
            "sample_count": self.samples,
        }


class CudaPeaks:
    """Track the maximum torch CUDA reserved/allocated pool over phases."""

    def __init__(self) -> None:
        self.reserved_peak = 0
        self.allocated_peak = 0

    def reset_phase(self) -> None:
        import torch

        torch.cuda.reset_peak_memory_stats()

    def record_phase(self) -> dict:
        import torch

        self.reserved_peak = max(
            self.reserved_peak, int(torch.cuda.max_memory_reserved())
        )
        self.allocated_peak = max(
            self.allocated_peak, int(torch.cuda.max_memory_allocated())
        )
        return {
            "cuda_reserved_peak_gib": round(
                int(torch.cuda.max_memory_reserved()) / 1024**3, 3
            ),
            "cuda_allocated_peak_gib": round(
                int(torch.cuda.max_memory_allocated()) / 1024**3, 3
            ),
        }

    def snapshot(self) -> dict:
        return {
            "cuda_reserved_peak_gib": round(self.reserved_peak / 1024**3, 3),
            "cuda_allocated_peak_gib": round(self.allocated_peak / 1024**3, 3),
        }


class BudgetExceeded(RuntimeError):
    """Raised when the measured generation exceeds the frozen Windows budget."""


def enforce_budget(
    sampler: ResourceSampler,
    cuda: CudaPeaks,
    elapsed_seconds: float,
    cuda_limit_gib: float = CUDA_RESERVED_LIMIT_GIB,
) -> None:
    """Fail closed when the measured generation exceeds the frozen budget."""
    resources = sampler.snapshot()
    gpu = cuda.snapshot()
    problems = []
    if resources["process_private_commit_peak_gib"] > PRIVATE_COMMIT_LIMIT_GIB:
        problems.append(
            "process private commit "
            f"{resources['process_private_commit_peak_gib']} GiB > "
            f"{PRIVATE_COMMIT_LIMIT_GIB} GiB"
        )
    if resources["system_available_ram_min_gib"] < AVAILABLE_RAM_MINIMUM_GIB:
        problems.append(
            "available RAM "
            f"{resources['system_available_ram_min_gib']} GiB < "
            f"{AVAILABLE_RAM_MINIMUM_GIB} GiB"
        )
    if gpu["cuda_reserved_peak_gib"] > cuda_limit_gib:
        problems.append(
            f"CUDA reserved {gpu['cuda_reserved_peak_gib']} GiB > "
            f"{cuda_limit_gib} GiB"
        )
    if elapsed_seconds > GENERATION_LIMIT_SECONDS:
        problems.append(
            f"generation {elapsed_seconds:.1f} s > {GENERATION_LIMIT_SECONDS:.0f} s"
        )
    if problems:
        raise BudgetExceeded("budget exceeded: " + "; ".join(problems))


def require_available_ram(phase: str, minimum_gib: float = AVAILABLE_RAM_MINIMUM_GIB) -> None:
    memory = system_memory()
    available_gib = memory["physical_available_bytes"] / 1024**3
    if available_gib < minimum_gib:
        raise BudgetExceeded(
            f"{phase}: available RAM {available_gib:.2f} GiB < {minimum_gib:.2f} GiB"
        )


def resource_report(sampler: ResourceSampler, cuda: CudaPeaks, elapsed_seconds: float) -> dict:
    return {
        "elapsed_seconds": round(elapsed_seconds, 2),
        **sampler.snapshot(),
        **cuda.snapshot(),
    }
