"""Release identity and immutable source evidence shared by benchmark runners."""

import json
import os
import sys
import zipfile

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
    actual, discovered = provenance.verify_release(native)
    assert actual == release and set(discovered) == set(inputs)
    override = tmp_path / "explicit-library.dll"
    override.write_bytes(native.read_bytes())
    assert provenance.verify_release(native, override)[0] == override
    native.write_bytes(b"different build")
    with pytest.raises(ValueError, match="must match"):
        provenance.verify_release(native)
    native.write_bytes(release.read_bytes())
    for path in inputs:
        os.utime(path, (300, 300))
        with pytest.raises(ValueError, match="newer"):
            provenance.verify_release(native)
        os.utime(path, (100, 100))


def test_archive_sources_and_change_detection_preserve_evidence(monkeypatch, tmp_path):
    monkeypatch.setattr(provenance, "ROOT", tmp_path)
    source = tmp_path / "source.py"
    source.write_text("original")
    hashes = {"source.py": provenance.sha256(source)}
    archive = tmp_path / "sources.zip"
    metadata = provenance.archive_sources(archive, [source, "source.py"])
    assert metadata == {
        "source_archive": archive.name,
        "source_archive_sha256": provenance.sha256(archive),
    }
    with zipfile.ZipFile(archive) as saved:
        assert saved.namelist() == ["source.py"]
        assert saved.read("source.py") == b"original"
    assert provenance.source_changes(hashes) == {
        "sources_unchanged_during_run": True,
        "changed_sources_during_run": [],
    }
    source.write_text("changed")
    assert provenance.source_changes(hashes) == {
        "sources_unchanged_during_run": False,
        "changed_sources_during_run": ["source.py"],
    }
    with pytest.raises(FileExistsError):
        provenance.archive_sources(archive, [source])
    assert provenance.sha256(archive) == metadata["source_archive_sha256"]


@pytest.mark.parametrize("suffix", [".csv", ".metadata.json", ".sources.zip"])
def test_core_runner_rejects_existing_evidence_before_writing(monkeypatch, tmp_path, suffix):
    monkeypatch.setattr(provenance, "ROOT", tmp_path)
    output = tmp_path / "run.csv"
    existing = output.with_suffix(suffix)
    existing.write_bytes(b"historical evidence")
    with pytest.raises(FileExistsError, match="refusing to overwrite"):
        provenance.run_cargo_bench(output, [], {}, [], {})
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
    source = tmp_path / "source.rs"
    source.write_text("original")
    output = tmp_path / "results/run.csv"
    hashes = {"source.rs": provenance.sha256(source)}
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
Path('source.rs').write_text('changed during run')
sys.exit(int(sys.argv[2]))
""",
        json.dumps(contents),
        str(exit_code),
    ]
    if launch_failure:
        invocation = [str(tmp_path / "missing-cargo")]
    result = provenance.run_cargo_bench(
        output,
        [source],
        {"BENCHMARK_CSV": str(output)},
        invocation,
        {"source_sha256": hashes, "scope": "test complete call"},
    )
    assert result == expected_code
    metadata = json.loads(output.with_suffix(".metadata.json").read_text())
    assert metadata["returncode"] == expected_code
    assert metadata["status"] == ("complete" if expected_code == 0 else "failed")
    assert metadata["scope"] == "test complete call"
    assert metadata["source_sha256"] == hashes
    assert metadata["changed_sources_during_run"] == ([] if launch_failure else ["source.rs"])
    archive = output.with_suffix(".sources.zip")
    assert metadata["source_archive_sha256"] == provenance.sha256(archive)
    with zipfile.ZipFile(archive) as saved:
        assert saved.read("source.rs") == b"original"
    if contents is not None:
        assert output.read_text() == contents
        assert metadata["raw_sha256"] == provenance.sha256(output)
    else:
        assert not output.exists()
        assert "raw_sha256" not in metadata
    if expected_code == 1:
        assert metadata["error"]
