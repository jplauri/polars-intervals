"""Release identity and immutable source evidence shared by benchmark runners."""

import os
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
