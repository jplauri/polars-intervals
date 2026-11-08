"""Shared benchmark machinery: run provenance and the common runners."""

import argparse
import csv
import ctypes
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
from datetime import UTC, datetime
from functools import partial
from itertools import product
from pathlib import Path
from time import perf_counter_ns

ROOT = Path(__file__).resolve().parents[1]
DTYPES = {"--dtypes": ("i64,u64,i16", {"i64", "u64", "i16"})}


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
    """Return the release library matching the installed plugin, or fail."""
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
    return release


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


def run_cargo_bench(output, settings, invocation, metadata):
    """Run a core benchmark and save its samples and metadata under a new prefix."""
    metadata_path = output.with_suffix(".metadata.json")
    for path in (output, metadata_path):
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
    metadata["status"] = "complete" if metadata["returncode"] == 0 else "failed"
    if output.exists():
        metadata["raw_sha256"] = sha256(output)
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    return metadata["returncode"]


def check_counts(parser, sizes, seeds, samples, warmups):
    if min(sizes) < 0 or min(samples, warmups) < 1:
        parser.error("Need nonnegative sizes and positive samples/warmups")
    if any(seed < 0 or seed > 2**64 - 1 for seed in seeds):
        parser.error("Seeds must fit UInt64")


def cpu_name():
    """Return the Windows processor name, which ``platform.processor`` omits."""
    query = "(Get-CimInstance Win32_Processor).Name"
    return command("powershell", "-NoProfile", "-Command", query)


def run_core(doc, bench, prefix, notes, selections=DTYPES, record_cpu=False):
    """Run one Rust core benchmark from the command line.

    ``selections`` maps each comma-separated flag to its default and allowed
    values. Every flag reaches the benchmark as a ``{prefix}_{FLAG}`` variable.
    ``notes`` is the runner's own metadata.
    """
    parser = argparse.ArgumentParser(description=doc)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--sizes", default="0,1,8,1000,10000,100000,1000000")
    parser.add_argument("--seeds", default="7,41")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    for flag, (default, _) in selections.items():
        parser.add_argument(flag, default=default)
    parser.add_argument("--cases", default="")
    args = parser.parse_args()
    try:
        sizes = [int(value) for value in args.sizes.split(",")]
        seeds = [int(value) for value in args.seeds.split(",")]
    except ValueError:
        parser.error("Sizes and seeds must be comma-separated integers")
    check_counts(parser, sizes, seeds, args.samples, args.warmups)
    for flag, (_, allowed) in selections.items():
        if not set(getattr(args, flag[2:].replace("-", "_")).split(",")) <= allowed:
            parser.error(f"{flag} must be chosen from {sorted(allowed)}")
    raw = args.output.resolve().with_suffix(".csv")
    settings = {f"{prefix}_CSV": str(raw)} | {
        f"{prefix}_{name.upper()}": str(value)
        for name, value in vars(args).items()
        if name != "output"
    }
    invocation = ["cargo", "bench", "-p", "intervals-core", "--bench", bench, "--locked"]
    if record_cpu and sys.platform == "win32":
        notes = notes | {"cpu_name": cpu_name()}
    try:
        returncode = run_cargo_bench(raw, settings, invocation, notes)
    except FileExistsError as error:
        parser.error(str(error))
    raise SystemExit(returncode)


def polars_parser(doc, cases, sizes, scopes=None, default_scopes=None):
    """Return the flags shared by end-to-end runners; add a runner's own before parsing."""
    parser = argparse.ArgumentParser(description=doc)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--sizes", nargs="+", type=int, default=sizes)
    parser.add_argument("--seeds", nargs="+", type=int, default=[7, 41])
    indices = range(len(cases))
    parser.add_argument("--cases", nargs="+", type=int, choices=indices, default=list(indices))
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    if scopes:
        parser.add_argument("--scopes", nargs="+", choices=scopes, default=list(default_scopes))
        parser.add_argument("--memory", action="store_true", help="Separate fresh-process RSS")
        parser.add_argument("--memory-worker", nargs="+", help=argparse.SUPPRESS)
    return parser


def memory_sample(values, cases, fixture, solve, size_column, memory_fields):
    """Measure one eager call in this fresh process and print the observation."""
    case, n, seed, *operation, method = values
    case, n, seed = int(case), int(n), int(seed)
    *frames, options = fixture(n, cases[case], seed)
    before = resident_memory()
    result = solve(*frames, *(operation or [None]), method, options)
    after = resident_memory()
    print(
        json.dumps(
            {
                "case": case,
                size_column: n,
                "seed": seed,
                **({"operation": operation[0]} if operation else {}),
                "method": method,
                "peak_increase_bytes": max(0, after["peak_rss_bytes"] - before["peak_rss_bytes"]),
                **memory_fields(frames, before, after, result),
            }
        )
    )


def memory_observation(name, *worker):
    """Run ``memory_sample`` for one call in a fresh process."""
    arguments = [sys.executable, "-m", f"benchmarks.{name}", "--memory-worker", *map(str, worker)]
    return json.loads(subprocess.check_output(arguments, cwd=ROOT, text=True))


def scoped_call(solve, scope, frames, sources, plans, operation, options, method):
    """Make one timed call. The scope decides whether planning or collection is included."""
    if scope == "eager":
        return solve(*frames, operation, method, options)
    if scope == "lazy_collect":
        return plans[method].collect()
    inputs = (frames[0], *sources[1:]) if scope == "mixed_complete" else sources
    plan = solve(*inputs, operation, method, options)
    if scope == "lazy_plan":
        return plan
    return plan.collect(engine="streaming" if scope == "streaming_complete" else "auto")


def run_polars(
    name,
    parser,
    args,
    cases,
    dimensions,
    fixture,
    solve,
    statistics,
    notes,
    *,
    operations=(None,),
    methods=lambda options: ["production", "native"],
    size_column="n",
    memory_fields=None,
    columns=None,
    omit=(),
    record_cpu=False,
):
    """Time complete calls, check each result and save samples and metadata.

    ``fixture(n, case, seed)`` returns the input frames, then the options.
    ``solve`` takes ``(*frames, operation, method, options)``. ``statistics``
    takes ``(*frames, options, operation, expected)`` and returns extra columns.
    ``operation`` is ``None`` unless the runner has ``operations``.

    A parser without scopes times eager calls only, has no scope column and
    verifies each method just before its warmups. ``columns`` fixes the CSV
    order. ``omit`` lists metadata keys this runner has never recorded, so new
    runs stay comparable with saved ones.
    """
    import polars as pl
    from polars.testing import assert_frame_equal
    from polars_intervals import _internal

    if getattr(args, "memory_worker", None):
        memory_sample(args.memory_worker, cases, fixture, solve, size_column, memory_fields)
        return
    if args.output is None:
        parser.error("Need --output")
    check_counts(parser, args.sizes, args.seeds, args.samples, args.warmups)
    native = Path(_internal.__file__)
    try:
        release = verify_release(native)
    except ValueError as error:
        parser.error(str(error))
    raw, meta, memory = [
        args.output.with_suffix(ext) for ext in (".csv", ".metadata.json", ".memory.json")
    ]
    if any(path.exists() for path in (raw, meta, memory)):
        parser.error("Use a new output prefix")
    raw.parent.mkdir(parents=True, exist_ok=True)
    metadata = environment() | {
        "settings": vars(args) | {"output": str(args.output)},
        "cases": cases,
        **notes,
        "native_path": str(native),
        "native_sha256": sha256(native),
        "release_path": str(release),
        "release_sha256": sha256(release),
        "build_environment": build_environment(),
        "status": "running",
    }
    if record_cpu and sys.platform == "win32":
        metadata["cpu_name"] = cpu_name()

    def save():
        kept = {key: value for key, value in metadata.items() if key not in omit}
        meta.write_text(json.dumps(kept, indent=2) + "\n", encoding="utf-8")

    def check(result, expected):
        assert_frame_equal(result, expected, rel_tol=1e-15, abs_tol=1e-15)

    scopes = getattr(args, "scopes", None)
    measure_memory = getattr(args, "memory", False)
    save()
    memories = []
    with (
        raw.open("w", newline="", encoding="utf-8") as stream,
        tempfile.TemporaryDirectory(prefix=f"{name}-") as temp,
    ):
        writer = None
        for case_id in args.cases:
            case = cases[case_id]
            for n, seed in product(args.sizes, args.seeds):
                *frames, options = fixture(n, case, seed)
                names = methods(options)
                sources = [frame.lazy() for frame in frames]
                parquet = case[-1] == "parquet"
                if parquet:
                    paths = [Path(temp) / f"{index}.parquet" for index in range(len(frames))]
                    for frame, path, divisor in zip(frames, paths, (7, 11)):
                        frame.write_parquet(path, row_group_size=max(1, frame.height // divisor))
                    sources = [pl.scan_parquet(path) for path in paths]
                for operation in operations:
                    expected = solve(*frames, operation, "production", options)
                    stats = statistics(*frames, options, operation, expected)
                    plans = {}
                    if scopes:
                        plans = {m: solve(*sources, operation, m, options) for m in names}
                    for plan in plans.values():
                        check(plan.collect(), expected)
                    labels = {"operation": operation} if operation else {}
                    for scope in scopes or ("eager",):
                        if parquet and scope in ("eager", "mixed_complete"):
                            continue
                        call = partial(
                            scoped_call, solve, scope, frames, sources, plans, operation, options
                        )
                        for method in names:
                            if not scopes:
                                check(call(method), expected)
                            for _ in range(args.warmups):
                                call(method)
                        for sample in range(args.samples):
                            turn = sample % len(names)
                            for method in names[turn:] + names[:turn]:
                                begin = perf_counter_ns()
                                result = call(method)
                                elapsed = perf_counter_ns() - begin
                                if scope == "lazy_plan":
                                    assert result.collect_schema() == expected.schema
                                else:
                                    check(result, expected)
                                del result
                                row = {
                                    "case": case_id,
                                    **dict(zip(dimensions, case)),
                                    "seed": seed,
                                    size_column: n,
                                    **labels,
                                    **({"scope": scope} if scopes else {}),
                                    "method": method,
                                    "sample": sample,
                                    "ns": elapsed,
                                    **stats,
                                }
                                if writer is None:
                                    writer = csv.DictWriter(stream, fieldnames=columns or list(row))
                                    writer.writeheader()
                                writer.writerow(row)
                    if measure_memory and case[-1] == "memory":
                        memories += [
                            memory_observation(name, case_id, n, seed, *labels.values(), method)
                            for method in names
                        ]
                stream.flush()
            print(f"case {case_id}: {case} complete", flush=True)
    if measure_memory:
        memory.write_text(json.dumps(memories, indent=2) + "\n", encoding="utf-8")
        metadata["memory_sha256"] = sha256(memory)
    metadata.update(status="complete", raw_sha256=sha256(raw))
    save()


def environment():
    """Machine, interpreter, toolchain and revision details for one run.

    Benchmarks measure committed code, so the revision is the source provenance.
    """
    import polars as pl

    changes = command("git", "status", "--porcelain", "--", ".", ":(exclude)benchmarks/results")
    if changes:
        raise SystemExit(f"Commit or stash changes before benchmarking:\n{changes}")
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
