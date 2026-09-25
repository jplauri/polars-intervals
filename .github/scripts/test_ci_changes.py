"""Keep documentation skips narrow without compiling the plugin."""

import importlib.util
import subprocess
from pathlib import Path

import pytest

spec = importlib.util.spec_from_file_location(
    "ci_changes", Path(__file__).with_name("ci_changes.py")
)
changes = importlib.util.module_from_spec(spec)
spec.loader.exec_module(changes)
PR = {"pull_request": {"base": {"sha": "base"}, "head": {"sha": "head"}}}


@pytest.mark.parametrize(
    ("paths", "required"),
    [
        (["README.md", "CONTRIBUTING.md", "mkdocs.yml", "benchmarks/README.md"], False),
        (["docs/usage.md", "docs/assets/example.png", "benchmarks/results/run.json"], False),
        (["README.md", "python/polars_intervals/__init__.py"], True),
        (["pyproject.toml"], True),
        (["uv.lock"], True),
        (["Cargo.lock"], True),
        (["crates/intervals-core/src/lib.rs"], True),
        (["tests/test_overlap_count.py"], True),
        (["benchmarks/overlap_count.py"], True),
        ([".github/workflows/python.yml"], True),
        (["LICENSE"], True),
        (["docs-helper.py"], True),
        (["new-build-config.toml"], True),
        ([], True),
    ],
)
def test_changed_paths(monkeypatch, paths, required):
    monkeypatch.setattr(
        changes.subprocess, "check_output", lambda *args, **kwargs: "".join(p + "\0" for p in paths)
    )
    assert changes.requires_build("pull_request", PR) is required


@pytest.mark.parametrize("event_name", ["push", "workflow_dispatch", "release"])
def test_other_events_always_build(event_name):
    assert changes.requires_build(event_name, {}) is True


def test_pr_diff_ignores_base_branch_changes_and_detects_renames(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)

    def git(*args):
        return subprocess.check_output(["git", *args], text=True).strip()

    git("init")
    git("config", "user.name", "CI test")
    git("config", "user.email", "ci-test@example.invalid")
    git("config", "commit.gpgsign", "false")
    (tmp_path / "README.md").write_text("Original docs\n")
    (tmp_path / "module.py").write_text("value = 1\n")
    git("add", ".")
    git("commit", "-m", "initial")
    ancestor = git("rev-parse", "HEAD")

    (tmp_path / "module.py").write_text("value = 2\n")
    git("commit", "-am", "base branch code change")
    base = git("rev-parse", "HEAD")
    git("checkout", "-b", "docs-change", ancestor)
    (tmp_path / "README.md").write_text("Updated docs\n")
    git("commit", "-am", "docs only")

    def event():
        return {"pull_request": {"base": {"sha": base}, "head": {"sha": git("rev-parse", "HEAD")}}}

    assert changes.requires_build("pull_request", event()) is False

    (tmp_path / "docs").mkdir()
    git("mv", "module.py", "docs/module.py")
    git("commit", "-m", "move source into docs")
    assert changes.requires_build("pull_request", event()) is True
