"""Run provenance shared by the benchmark runners: environment and file hashes."""

import ctypes
import hashlib
import os
import platform
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def sha256(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def command(*args):
    """Return a command's output, or why it is unavailable."""
    try:
        return subprocess.check_output(args, cwd=ROOT, text=True, stderr=subprocess.STDOUT).strip()
    except (OSError, subprocess.CalledProcessError) as error:
        return f"unavailable: {error}"


def environment():
    """Machine, interpreter, toolchain and revision details for one run."""
    import polars as pl

    return {
        "timestamp_utc": datetime.now(UTC).isoformat(),
        "command": sys.argv,
        "platform": platform.platform(),
        "processor": platform.processor(),
        "logical_cpus": os.cpu_count(),
        "python": sys.version,
        "executable": sys.executable,
        "polars": pl.__version__,
        "polars_threads": pl.thread_pool_size(),
        "thread_environment": {
            name: os.environ.get(name)
            for name in ("POLARS_MAX_THREADS", "RAYON_NUM_THREADS", "CARGO_BUILD_JOBS")
        },
        "rustc": command("rustc", "-Vv"),
        "revision": command("git", "rev-parse", "HEAD"),
        "git_status": command("git", "status", "--porcelain"),
    }


def resident_memory() -> dict:
    """OS process high-water mark; no sampling thread and no extra dependency."""
    if sys.platform == "win32":
        from ctypes import wintypes

        class Counters(ctypes.Structure):
            _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [
                (name, ctypes.c_size_t)
                for name in (
                    "PeakWorkingSetSize",
                    "WorkingSetSize",
                    "QuotaPeakPagedPoolUsage",
                    "QuotaPagedPoolUsage",
                    "QuotaPeakNonPagedPoolUsage",
                    "QuotaNonPagedPoolUsage",
                    "PagefileUsage",
                    "PeakPagefileUsage",
                )
            ]

        counters = Counters()
        counters.cb = ctypes.sizeof(counters)
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = wintypes.HANDLE
        psapi = ctypes.WinDLL("psapi", use_last_error=True)
        psapi.GetProcessMemoryInfo.argtypes = [
            wintypes.HANDLE,
            ctypes.POINTER(Counters),
            wintypes.DWORD,
        ]
        if not psapi.GetProcessMemoryInfo(
            kernel.GetCurrentProcess(), ctypes.byref(counters), counters.cb
        ):
            raise ctypes.WinError(ctypes.get_last_error())
        return {"rss_bytes": counters.WorkingSetSize, "peak_rss_bytes": counters.PeakWorkingSetSize}
    import resource

    scale = 1 if sys.platform == "darwin" else 1024
    return {
        "rss_bytes": None,
        "peak_rss_bytes": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss * scale,
    }
