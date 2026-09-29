"""Run provenance shared by the benchmark runners: environment and file hashes."""

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
