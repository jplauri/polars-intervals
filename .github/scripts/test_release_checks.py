"""Exercise release gates without network access or native builds."""

import importlib.util
import json
from pathlib import Path

import pytest

spec = importlib.util.spec_from_file_location(
    "release_checks", Path(__file__).with_name("release_checks.py")
)
checks = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checks)
SHA = "a" * 40


@pytest.mark.parametrize(
    ("core", "plugin", "error"),
    [
        ("1.2.3", "1.2.3", None),
        ("1.2.2", "1.2.3", "same version"),
        ("1.2.3rc1", "1.2.3rc1", "X.Y.Z"),
        ("01.2.3", "01.2.3", "X.Y.Z"),
    ],
)
def test_versions(tmp_path, core, plugin, error):
    for crate, version in (("intervals-core", core), ("polars-intervals", plugin)):
        directory = tmp_path / "crates" / crate
        directory.mkdir(parents=True)
        (directory / "Cargo.toml").write_text(f'[package]\nversion = "{version}"\n')
    (tmp_path / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.95.0"\n')
    if error:
        with pytest.raises(SystemExit, match=error):
            checks.versions(tmp_path)
    else:
        assert checks.versions(tmp_path) == {"version": "1.2.3", "rust_toolchain": "1.95.0"}


@pytest.fixture
def ci():
    runs = [
        {
            "id": number,
            "path": f".github/workflows/{workflow}.yml",
            "head_sha": SHA,
            "event": "push",
            "head_branch": "master",
            "status": "completed",
            "conclusion": "success",
        }
        for number, workflow in enumerate(("rust", "python"), 1)
    ]
    jobs = {
        number: [{"name": name, "status": "completed", "conclusion": "success"} for name in names]
        for number, names in enumerate(
            (
                ("checks",),
                ("quality", "Python 3.12 tests", "Python 3.13 tests", "Python 3.14 tests"),
            ),
            1,
        )
    }
    return runs, jobs


def test_ci_passes(ci):
    runs, jobs = ci
    checks.check_ci(SHA, runs, jobs.__getitem__)


@pytest.mark.parametrize("conclusion", ["failure", None])
def test_ci_latest_run_supersedes_older_success(ci, conclusion):
    runs, jobs = ci
    runs.append({**runs[0], "id": 3, "conclusion": conclusion})
    with pytest.raises(SystemExit, match="Latest .* successfully"):
        checks.check_ci(SHA, runs, jobs.__getitem__)


@pytest.mark.parametrize(
    ("field", "value"),
    [("head_sha", "b" * 40), ("event", "pull_request"), ("head_branch", "feature")],
)
def test_ci_requires_master_push_provenance(ci, field, value):
    runs, jobs = ci
    runs[0][field] = value
    with pytest.raises(SystemExit, match="No master push CI run"):
        checks.check_ci(SHA, runs, jobs.__getitem__)


@pytest.mark.parametrize("conclusion", ["missing", "skipped", "failure", None])
def test_ci_requires_every_successful_job(ci, conclusion):
    runs, jobs = ci
    if conclusion == "missing":
        jobs[2].pop()
    else:
        jobs[2][-1]["conclusion"] = conclusion
    with pytest.raises(SystemExit, match="Missing successful jobs"):
        checks.check_ci(SHA, runs, jobs.__getitem__)


@pytest.fixture
def release_event(tmp_path, monkeypatch, ci):
    event = {
        "action": "published",
        "repository": {"full_name": checks.REPOSITORY},
        "release": {"draft": False, "prerelease": False, "tag_name": "v1.2.3"},
    }
    path = tmp_path / "event.json"
    path.write_text(json.dumps(event))
    monkeypatch.setenv("GITHUB_EVENT_NAME", "release")
    monkeypatch.setenv("GITHUB_EVENT_PATH", str(path))
    monkeypatch.setenv("GITHUB_SHA", SHA)
    monkeypatch.setattr(checks, "versions", lambda: {"version": "1.2.3"})
    monkeypatch.setattr(checks.subprocess, "check_output", lambda *args, **kwargs: SHA + "\n")
    comparison = {"status": "identical"}
    monkeypatch.setattr(checks, "api", lambda path: comparison)
    runs, jobs = ci
    monkeypatch.setattr(
        checks,
        "pages",
        lambda path, key: runs if key == "workflow_runs" else jobs[int(path.split("/")[2])],
    )
    return event, path, comparison


@pytest.mark.parametrize("status", ["ahead", "identical"])
def test_release_accepts_ci_passing_master_commit(release_event, status):
    release_event[2]["status"] = status
    checks.release()


def test_release_refuses_manual_event(release_event, monkeypatch):
    monkeypatch.setenv("GITHUB_EVENT_NAME", "workflow_dispatch")
    with pytest.raises(SystemExit, match="Only release events"):
        checks.release()


@pytest.mark.parametrize(
    ("field", "value", "message"),
    [
        ("prerelease", True, "final releases"),
        ("draft", True, "final releases"),
        ("tag_name", "v9.9.9", "Cargo version"),
        ("tag_name", "1.2.3", "Cargo version"),
    ],
)
def test_release_refuses_invalid_release(release_event, field, value, message):
    event, path, _ = release_event
    event["release"][field] = value
    path.write_text(json.dumps(event))
    with pytest.raises(SystemExit, match=message):
        checks.release()


@pytest.mark.parametrize("ref", ["HEAD", "refs/tags/v1.2.3^{commit}"])
def test_release_requires_exact_checkout_and_tag(release_event, monkeypatch, ref):
    monkeypatch.setattr(
        checks.subprocess,
        "check_output",
        lambda command, **kwargs: "b" * 40 if command[-1] == ref else SHA,
    )
    with pytest.raises(SystemExit, match="event's exact commit"):
        checks.release()


@pytest.mark.parametrize("status", ["behind", "diverged"])
def test_release_refuses_commit_outside_master(release_event, status):
    release_event[2]["status"] = status
    with pytest.raises(SystemExit, match="belong to master"):
        checks.release()


def test_artifacts_requires_complete_wheel_matrix(tmp_path, monkeypatch):
    monkeypatch.setattr(checks, "versions", lambda: {"version": "1.2.3"})
    with pytest.raises(SystemExit, match="all 15 CPython/platform wheels"):
        checks.artifacts(tmp_path)
