# Releasing

Releases are deliberate maintainer actions. The
[Release workflow](https://github.com/jplauri/polars-intervals/blob/master/.github/workflows/release.yml)
builds and verifies distributions on pull requests that change code or build
inputs, and on every manual run, without publishing. Documentation-only pull
requests run the configuration and CI helper checks without building distributions.
Publishing a non-prerelease GitHub release also builds and verifies
the distributions, then publishes those same artifacts to PyPI through OIDC.

## One-time setup

Create a GitHub environment named `pypi` in the repository settings. On PyPI,
configure a pending Trusted Publisher for the first release:

| Field | Value |
| --- | --- |
| PyPI project name | `polars-intervals` |
| GitHub owner | `jplauri` |
| Repository | `polars-intervals` |
| Workflow filename | `release.yml` |
| Environment | `pypi` |

For an existing PyPI project, configure the same Trusted Publisher in its
publishing settings. These settings must match the workflow before publishing.

## Rehearse without publishing

In GitHub Actions, select **Release**, choose **Run workflow**, and select the
branch to verify. A manual run never publishes, even when run on a release tag.

Review the complete run and download its distribution artifacts. The workflow
is the source of truth for the platform and Python matrix. Every expected wheel
must be present, installed in an isolated environment, and pass the Python tests,
API doctests, and a real `overlap_count` smoke test. The sdist must independently
build and install outside the checkout and pass its smoke test. Artifact metadata
checks must also pass.

The sdist includes `rust-toolchain.toml` and `Cargo.lock`; release builds use the
pinned Rust toolchain and locked dependencies. An import from the checkout is not
evidence that an installed distribution works. Do not advertise a platform until
its artifact verification passes.

## Publish a version

1. Prepare the version and [release notes](changelog.md) in a reviewed PR.
   Update `crates/polars-intervals/Cargo.toml` and
   `crates/intervals-core/Cargo.toml` to the same release version. The Python
   package version comes from the former manifest; the workflow does not bump
   versions. Describe user-visible changes, installation, and compatibility.
2. Run the [development checks](contributing.md#checks)
   and `uv lock --check`. Preserve the locked Rust checks and strict Rust
   documentation check, as well as the Python and strict MkDocs checks. Merge the
   reviewed changes, require normal CI to pass on the final `master` commit, and
   complete a manual release rehearsal for that commit.
3. Create the tag at that exact reviewed commit. Its name must be `vX.Y.Z`,
   matching the package version exactly: `v0.1.0` for `0.1.0`. Publish a GitHub
   release using that tag and the release notes. Leave **Set as a pre-release**
   unchecked. Pushing a tag alone does not publish to PyPI.
4. Watch the release workflow. Only a published, non-prerelease GitHub release
   can reach the `pypi` publishing job, after successful artifact verification.
   That job downloads the verified artifacts and publishes them without
   rebuilding.
5. Verify the published version from PyPI as described below. Inspect its project
   page and distribution files as well as the installation result.

Repeat these steps for future releases, updating versions, release notes, and
documentation before tagging. Do not change API behavior as part of the release
process itself.

## Verify the PyPI installation

After publishing, run this outside the repository using a supported Python
version. It creates an isolated environment and verifies actual overlap counts:

```sh
uv run --isolated --no-project --with polars-intervals==0.1.0 python -c "import polars as pl; import polars_intervals as pi; df = pl.DataFrame({'start': [1, 3, 2, 2], 'end': [3, 5, 4, 2]}); actual = df.select(pi.overlap_count('start', 'end')).to_series().to_list(); assert actual == [1, 1, 2, 0], actual; print(actual)"
```

Use the newly published version in place of `0.1.0` for subsequent releases.
