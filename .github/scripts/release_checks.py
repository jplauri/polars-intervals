"""Validate release inputs and distributions without publishing anything."""

import argparse
import json
import os
import re
import subprocess
import tarfile
import tempfile
import tomllib
import urllib.request
import zipfile
from email.parser import BytesParser
from importlib.metadata import version as installed_version
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = "jplauri/polars-intervals"
REQUIRED_JOBS = {
    ".github/workflows/rust.yml": {"checks"},
    ".github/workflows/python.yml": {
        "quality",
        "Python 3.12 tests",
        "Python 3.13 tests",
        "Python 3.14 tests",
    },
}


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def versions(root=ROOT):
    manifests = [
        tomllib.loads((root / "crates" / crate / "Cargo.toml").read_text(encoding="utf-8"))
        for crate in ("intervals-core", "polars-intervals")
    ]
    version = manifests[1]["package"]["version"]
    require(
        re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", version),
        "Release versions must use X.Y.Z without a prerelease suffix.",
    )
    require(
        manifests[0]["package"]["version"] == version,
        "Both crates must have the same version for this release process.",
    )
    toolchain = tomllib.loads((root / "rust-toolchain.toml").read_text(encoding="utf-8"))
    return {"version": version, "rust_toolchain": toolchain["toolchain"]["channel"]}


def api(path):
    request = urllib.request.Request(
        f"https://api.github.com/repos/{REPOSITORY}/{path}",
        headers={
            "Authorization": f"Bearer {os.environ['GH_TOKEN']}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "polars-intervals-release-checks",
        },
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def pages(path, key):
    separator = "&" if "?" in path else "?"
    page = 1
    while True:
        items = api(f"{path}{separator}per_page=100&page={page}")[key]
        yield from items
        if len(items) < 100:
            return
        page += 1


def check_ci(sha, runs, jobs_for_run):
    for workflow, expected_jobs in REQUIRED_JOBS.items():
        matching = [
            run
            for run in runs
            if run["path"] == workflow
            and run["head_sha"] == sha
            and run["event"] == "push"
            and run["head_branch"] == "master"
        ]
        require(matching, f"No master push CI run for {workflow} at {sha}.")
        run = max(matching, key=lambda item: item["id"])
        require(
            run["status"] == "completed" and run["conclusion"] == "success",
            f"Latest {workflow} run must finish successfully before publishing.",
        )
        passed = {
            job["name"]
            for job in jobs_for_run(run["id"])
            if job["status"] == "completed" and job["conclusion"] == "success"
        }
        require(expected_jobs <= passed, f"Missing successful jobs: {expected_jobs - passed}.")


def release():
    require(os.environ.get("GITHUB_EVENT_NAME") == "release", "Only release events may publish.")
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
    require(
        event["action"] == "published"
        and not event["release"]["prerelease"]
        and not event["release"]["draft"],
        "Only published, final releases may publish.",
    )
    require(event["repository"]["full_name"] == REPOSITORY, "Unexpected release repository.")
    tag = event["release"]["tag_name"]
    require(tag == f"v{versions()['version']}", "Release tag does not match the Cargo version.")
    sha = os.environ["GITHUB_SHA"]
    for ref in ("HEAD", f"refs/tags/{tag}^{{commit}}"):
        resolved = subprocess.check_output(["git", "rev-parse", "--verify", ref], text=True).strip()
        require(resolved == sha, f"{ref} must identify the event's exact commit.")
    comparison = api(f"compare/{sha}...master")
    require(comparison["status"] in {"ahead", "identical"}, "Release commit must belong to master.")
    runs = list(pages(f"actions/runs?head_sha={sha}&event=push&branch=master", "workflow_runs"))
    check_ci(sha, runs, lambda run_id: pages(f"actions/runs/{run_id}/jobs?filter=latest", "jobs"))
    print(f"Verified final release {tag} from CI-passing master commit {sha}.")


def normalized_text(data):
    return data.decode("utf-8").replace("\r\n", "\n")


def check_metadata(data, version):
    metadata = BytesParser().parsebytes(data)
    project = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))["project"]
    for field, expected in {
        "Name": project["name"],
        "Version": version,
        "Requires-Python": project["requires-python"],
        "License-Expression": project["license"],
    }.items():
        require(metadata[field] == expected, f"Incorrect distribution metadata: {field}.")
    media_type = metadata.get("Description-Content-Type", "").split(";", 1)[0].strip().lower()
    require(media_type == "text/markdown", "Distribution description must be Markdown.")
    description = normalized_text(metadata.get_payload(decode=True))
    require(
        description.strip() == (ROOT / "README.md").read_text(encoding="utf-8").strip(),
        "Distribution README differs from the release checkout.",
    )


def artifacts(directory):
    version = versions()["version"]
    wheels = sorted(directory.glob("*.whl"))
    sdists = list(directory.glob("*.tar.gz"))
    require(
        len(wheels) == 15 and len(sdists) == 1,
        "Expected all 15 CPython/platform wheels and exactly one source archive.",
    )
    expected = {
        (python, platform)
        for python in ("cp312", "cp313", "cp314")
        for platform in ("linux-x86_64", "linux-aarch64", "macos-x86_64", "macos-arm64", "windows")
    }
    actual = set()
    for wheel in wheels:
        package, python, abi, platform = wheel.stem.rsplit("-", 3)
        require(
            package == f"polars_intervals-{version}" and python == abi,
            f"Unexpected wheel version or ABI: {wheel.name}.",
        )
        if platform.startswith("manylinux"):
            target = "linux-aarch64" if platform.endswith("_aarch64") else "linux-x86_64"
            require(platform.endswith(("_aarch64", "_x86_64")), "Unsupported Linux architecture.")
        elif platform.startswith("macosx_"):
            target = "macos-arm64" if platform.endswith("_arm64") else "macos-x86_64"
            require(platform.endswith(("_arm64", "_x86_64")), "Unsupported macOS architecture.")
        else:
            require(platform == "win_amd64", f"Unexpected platform: {platform}.")
            target = "windows"
        require((python, target) not in actual, f"Duplicate wheel target: {python}/{target}.")
        actual.add((python, target))
        with zipfile.ZipFile(wheel) as archive:
            names = archive.namelist()
            require("polars_intervals/py.typed" in names, "Wheel is missing py.typed.")
            metadata = f"polars_intervals-{version}.dist-info/METADATA"
            check_metadata(archive.read(metadata), version)
            license_path = f"polars_intervals-{version}.dist-info/licenses/LICENSE"
            require(
                normalized_text(archive.read(license_path))
                == (ROOT / "LICENSE").read_text(encoding="utf-8"),
                "License mismatch.",
            )
    require(actual == expected, f"Incorrect wheel coverage; missing {expected - actual}.")
    sdist = sdists[0]
    require(sdist.name == f"polars_intervals-{version}.tar.gz", "Incorrect source archive version.")
    with tarfile.open(sdist) as archive:
        prefix = f"polars_intervals-{version}/"
        for name in ("rust-toolchain.toml", "Cargo.lock", "LICENSE"):
            member = archive.extractfile(prefix + name)
            require(
                member is not None
                and normalized_text(member.read()) == (ROOT / name).read_text(encoding="utf-8"),
                f"Source archive must preserve {name}.",
            )
        check_metadata(archive.extractfile(prefix + "PKG-INFO").read(), version)
    print("Verified all 15 wheels and the source archive, including versions, README and license.")


def installed(checkout):
    import polars as pl
    import polars_intervals as pi
    import pytest

    checkout = checkout.resolve()
    package = Path(pi.__file__).resolve().parent
    require(not package.is_relative_to(checkout), "Imported the checkout instead of the artifact.")
    require(
        installed_version("polars-intervals") == versions(checkout)["version"],
        "Installed package version differs from the release version.",
    )
    result = (
        pl.DataFrame({"start": [1, 3, 2, 2], "end": [3, 5, 4, 2]})
        .select(pi.overlap_count("start", "end"))
        .to_series()
    )
    require(result.dtype == pl.UInt64 and result.to_list() == [1, 1, 2, 0], "Smoke test failed.")
    print(f"Testing installed artifact from {package}")
    with tempfile.TemporaryDirectory() as temporary:
        previous_directory = Path.cwd()
        try:
            os.chdir(temporary)
            result = pytest.main(
                [
                    "--rootdir",
                    temporary,
                    "--import-mode=importlib",
                    "--doctest-modules",
                    str(package),
                    str(checkout / "tests"),
                    "-q",
                ]
            )
        finally:
            os.chdir(previous_directory)
    raise SystemExit(result)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("versions")
    commands.add_parser("release")
    commands.add_parser("artifacts").add_argument("directory", type=Path)
    commands.add_parser("installed").add_argument("--checkout", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "versions":
        values = versions()
        print(json.dumps(values))
        if output := os.environ.get("GITHUB_OUTPUT"):
            with Path(output).open("a", encoding="utf-8") as stream:
                stream.writelines(f"{key}={value}\n" for key, value in values.items())
    elif args.command == "release":
        release()
    elif args.command == "artifacts":
        artifacts(args.directory)
    else:
        installed(args.checkout)
