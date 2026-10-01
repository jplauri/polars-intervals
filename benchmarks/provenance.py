"""Run provenance shared by the benchmark runners: environment and file hashes."""

import ctypes
import hashlib
import json
import os
import platform
import subprocess
import sys
import zipfile
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


def verify_release(native, release=None):
    """Return the matching release library and its source inputs, or fail."""
    release = release or next(
        (
            ROOT / "target/release" / name
            for name in (
                "polars_intervals.dll",
                "libpolars_intervals.so",
                "libpolars_intervals.dylib",
            )
            if (ROOT / "target/release" / name).is_file()
        ),
        None,
    )
    if release is None or not release.is_file() or sha256(native) != sha256(release):
        raise ValueError("Installed plugin must match the fresh Cargo release library")
    inputs = [
        *ROOT.glob("crates/*/src/**/*.rs"),
        *ROOT.glob("crates/*/Cargo.toml"),
        *(ROOT / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml")),
    ]
    if any(path.stat().st_mtime_ns > release.stat().st_mtime_ns for path in inputs):
        raise ValueError("Rust inputs are newer than the release library; rebuild release first")
    return release, inputs


def build_environment():
    return {
        name: os.environ.get(name)
        for name in (
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_PROFILE_RELEASE_OPT_LEVEL",
            "CARGO_PROFILE_RELEASE_LTO",
            "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
        )
    }


def archive_sources(archive, paths):
    """Create a source archive without overwriting an existing measurement."""
    with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as saved:
        for path in dict.fromkeys(ROOT / path for path in paths):
            saved.write(path, path.relative_to(ROOT))
    return {"source_archive": archive.name, "source_archive_sha256": sha256(archive)}


def source_changes(hashes):
    changed = [path for path, digest in hashes.items() if sha256(ROOT / path) != digest]
    return {"sources_unchanged_during_run": not changed, "changed_sources_during_run": changed}


def run_cargo_bench(output, paths, settings, invocation, metadata):
    """Run a core benchmark and save its samples and source evidence under a new prefix."""
    metadata_path = output.with_suffix(".metadata.json")
    archive_path = output.with_suffix(".sources.zip")
    for path in (output, metadata_path, archive_path):
        if path.exists():
            raise FileExistsError(f"refusing to overwrite historical evidence: {path}")
    output.parent.mkdir(parents=True, exist_ok=True)
    metadata = (
        environment()
        | {
            "settings": settings,
            "cargo_command": invocation,
            "cargo_version": command("cargo", "--version"),
            "build_environment": build_environment(),
            "status": "running",
        }
        | metadata
    )
    metadata.update(archive_sources(archive_path, paths))
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    try:
        result = subprocess.run(invocation, cwd=ROOT, env={**os.environ, **settings}, check=False)
        metadata["returncode"] = result.returncode
    except OSError as error:
        metadata.update(returncode=1, error=str(error))
    if metadata["returncode"] == 0:
        if output.exists():
            with output.open(encoding="utf-8") as raw:
                next(raw, None)
                has_samples = any(line.strip() for line in raw)
        else:
            has_samples = False
        if not has_samples:
            metadata.update(
                returncode=1, error="No samples emitted; check selected cases and sizes"
            )
    metadata.update(source_changes(metadata["source_sha256"]))
    metadata["status"] = "complete" if metadata["returncode"] == 0 else "failed"
    if output.exists():
        metadata["raw_sha256"] = sha256(output)
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    return metadata["returncode"]


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
