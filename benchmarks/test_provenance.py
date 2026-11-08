"""Release identity, clean-tree provenance and shared-runner evidence."""

import importlib
import json
import os
import sys
from pathlib import Path

import pytest

from benchmarks import provenance


def test_verify_release_checks_identity_and_all_rust_inputs(monkeypatch, tmp_path):
    monkeypatch.setattr(provenance, "ROOT", tmp_path)
    inputs = [
        tmp_path / name
        for name in (
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "crates/core/Cargo.toml",
            "crates/core/src/lib.rs",
        )
    ]
    for path in inputs:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("source")
        os.utime(path, (100, 100))
    native = tmp_path / "installed.pyd"
    native.write_bytes(b"release")
    with pytest.raises(ValueError, match="must match"):
        provenance.verify_release(native)
    release = tmp_path / "target/release/polars_intervals.dll"
    release.parent.mkdir(parents=True)
    release.write_bytes(native.read_bytes())
    os.utime(release, (200, 200))
    assert provenance.verify_release(native) == release
    override = tmp_path / "explicit-library.dll"
    override.write_bytes(native.read_bytes())
    assert provenance.verify_release(native, override) == override
    native.write_bytes(b"different build")
    with pytest.raises(ValueError, match="must match"):
        provenance.verify_release(native)
    native.write_bytes(release.read_bytes())
    for path in inputs:
        os.utime(path, (300, 300))
        with pytest.raises(ValueError, match="newer"):
            provenance.verify_release(native)
        os.utime(path, (100, 100))


def test_environment_refuses_uncommitted_changes(monkeypatch):
    outputs = {"status": " M crates/intervals-core/src/lib.rs", "rev-parse": "abc123"}
    monkeypatch.setattr(provenance, "command", lambda *args: outputs.get(args[1], "tool"))
    with pytest.raises(SystemExit, match="Commit or stash"):
        provenance.environment()
    outputs["status"] = ""
    assert provenance.environment()["revision"] == "abc123"


@pytest.mark.parametrize("suffix", [".csv", ".metadata.json"])
def test_core_runner_rejects_existing_evidence_before_writing(monkeypatch, tmp_path, suffix):
    monkeypatch.setattr(provenance, "ROOT", tmp_path)
    output = tmp_path / "run.csv"
    existing = output.with_suffix(suffix)
    existing.write_bytes(b"historical evidence")
    with pytest.raises(FileExistsError, match="refusing to overwrite"):
        provenance.run_cargo_bench(output, {}, [], {})
    assert list(tmp_path.iterdir()) == [existing]
    assert existing.read_bytes() == b"historical evidence"


@pytest.mark.parametrize(
    "contents,exit_code,launch_failure,expected_code",
    [
        ("sample,ns\n0,42\n", 0, False, 0),
        ("sample,ns\n0,42\n", 7, False, 7),
        (None, 0, False, 1),
        ("", 0, False, 1),
        ("sample,ns\n\n", 0, False, 1),
        (None, 0, True, 1),
    ],
    ids=["success", "partial-failure", "missing", "empty", "no-samples", "launch-failure"],
)
def test_core_runner_preserves_evidence_and_finalizes_status(
    monkeypatch, tmp_path, contents, exit_code, launch_failure, expected_code
):
    monkeypatch.setattr(provenance, "ROOT", tmp_path)
    monkeypatch.setattr(provenance, "environment", lambda: {"revision": "test revision"})
    monkeypatch.setattr(provenance, "command", lambda *args: "test cargo version")
    output = tmp_path / "results/run.csv"
    invocation = [
        sys.executable,
        "-c",
        """
import json, os, sys
from pathlib import Path
output = Path(os.environ['BENCHMARK_CSV'])
assert json.loads(output.with_suffix('.metadata.json').read_text())['status'] == 'running'
contents = json.loads(sys.argv[1])
if contents is not None:
    output.write_text(contents)
sys.exit(int(sys.argv[2]))
""",
        json.dumps(contents),
        str(exit_code),
    ]
    if launch_failure:
        invocation = [str(tmp_path / "missing-cargo")]
    result = provenance.run_cargo_bench(
        output, {"BENCHMARK_CSV": str(output)}, invocation, {"scope": "test complete call"}
    )
    assert result == expected_code
    metadata = json.loads(output.with_suffix(".metadata.json").read_text())
    assert metadata["returncode"] == expected_code
    assert metadata["status"] == ("complete" if expected_code == 0 else "failed")
    assert metadata["scope"] == "test complete call"
    assert metadata["revision"] == "test revision"
    if contents is not None:
        assert output.read_text() == contents
        assert metadata["raw_sha256"] == provenance.sha256(output)
    else:
        assert not output.exists()
        assert "raw_sha256" not in metadata
    if expected_code == 1:
        assert metadata["error"]


RESULTS = Path(__file__).parent / "results"
RETIRED_KEYS = {
    "git_status",
    "source_sha256",
    "source_archive",
    "source_archive_sha256",
    "sources_unchanged_during_run",
    "changed_sources_during_run",
}


def saved_metadata(stem):
    return json.loads((RESULTS / f"{stem}.metadata.json").read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    "runner,saved,flags",
    [
        ("interval_geometry", "interval-geometry-polars-20260929", []),
        ("set_geometry", "set-geometry-polars-repeat-20261001", []),
        ("coverage_stats", "coverage-stats-polars-local", []),
        ("coverage_profile", "coverage-profile-lazy-20260929", ["--lazy"]),
    ],
)
def test_polars_runners_keep_saved_columns_and_keys(monkeypatch, tmp_path, runner, saved, flags):
    module = importlib.import_module(f"benchmarks.{runner}")
    monkeypatch.setattr(provenance, "command", lambda *args: "")
    monkeypatch.setattr(provenance, "verify_release", Path)
    output = tmp_path / "run"
    arguments = ["--sizes", "8", "--seeds", "7", "--cases", "0", "--samples", "1", "--warmups", "1"]
    monkeypatch.setattr(sys, "argv", [runner, "--output", str(output), *arguments, *flags])
    module.main()
    headers = [
        path.read_text(encoding="utf-8").splitlines()[0]
        for path in (output.with_suffix(".csv"), RESULTS / f"{saved}.csv")
    ]
    assert headers[0] == headers[1]
    fresh = json.loads(output.with_suffix(".metadata.json").read_text(encoding="utf-8"))
    recorded = saved_metadata(saved)
    assert fresh["status"] == "complete"
    assert set(fresh) - {"cpu_name"} == set(recorded) - RETIRED_KEYS - {"cpu_name"}
    assert set(fresh["settings"]) == set(recorded["settings"])


@pytest.mark.parametrize(
    "runner,saved,worker",
    [
        ("interval_geometry", "interval-geometry-polars-memory-20260929", "0 8 7 merge native"),
        ("set_geometry", "set-geometry-polars-memory-20261001", "0 8 7 subtract native"),
        ("coverage_stats", "coverage-stats-polars-modes", "0 8 7 native"),
    ],
)
def test_memory_workers_keep_saved_fields(monkeypatch, capsys, runner, saved, worker):
    module = importlib.import_module(f"benchmarks.{runner}")
    monkeypatch.setattr(sys, "argv", [runner, "--memory-worker", *worker.split()])
    module.main()
    recorded = json.loads((RESULTS / f"{saved}.memory.json").read_text(encoding="utf-8"))
    assert set(json.loads(capsys.readouterr().out)) == set(recorded[0])


@pytest.mark.parametrize(
    "runner,saved",
    [
        ("interval_geometry_core", "interval-geometry-core-final-20260929"),
        ("set_geometry_core", "set-geometry-core-million-20260930"),
        ("coverage_stats_core", "coverage-stats-core-production"),
        ("coverage_profile_core", "coverage-profile-core-final-20260929"),
    ],
)
def test_core_runners_keep_saved_settings(monkeypatch, tmp_path, runner, saved):
    monkeypatch.syspath_prepend(str(Path(__file__).parent))
    module = importlib.import_module(runner)
    calls = []

    def record(output, settings, invocation, metadata):
        calls.append((output, settings, invocation))
        return 0

    monkeypatch.setitem(module.run_core.__globals__, "run_cargo_bench", record)
    monkeypatch.setitem(module.run_core.__globals__, "command", lambda *args: "")
    monkeypatch.setattr(sys, "argv", [runner, "--output", str(tmp_path / "run")])
    with pytest.raises(SystemExit) as error:
        module.main()
    assert error.value.code == 0
    output, settings, invocation = calls[0]
    recorded = saved_metadata(saved)
    assert output.name == "run.csv"
    assert set(settings) == set(recorded["settings"])
    assert invocation == recorded["cargo_command"]
