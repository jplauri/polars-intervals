# Releasing

Releases are deliberate maintainer actions. The
[Release workflow](https://github.com/jplauri/polars-intervals/blob/master/.github/workflows/release.yml)
builds and verifies distributions on pull requests and manual runs without
publishing. Publishing a non-prerelease GitHub release also builds and verifies
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

1. Prepare the version and release notes in a reviewed PR. For `v0.1.0`, both
   `crates/polars-intervals/Cargo.toml` and `crates/intervals-core/Cargo.toml` declare
   `0.1.0`. The Python package version comes from the former manifest. For future
   releases, update crate versions deliberately and keep them compatible; the
   workflow does not bump versions.
2. Run the [development checks](https://github.com/jplauri/polars-intervals#development-and-documentation)
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

After the first release, run this outside the repository using a supported Python
version. It creates an isolated environment and verifies actual overlap counts:

```sh
uv run --isolated --no-project --with polars-intervals==0.1.0 python -c "import polars as pl; import polars_intervals as pi; df = pl.DataFrame({'start': [1, 3, 2, 2], 'end': [3, 5, 4, 2]}); actual = df.select(pi.overlap_count('start', 'end')).to_series().to_list(); assert actual == [1, 1, 2, 0], actual; print(actual)"
```

Use the newly published version in place of `0.1.0` for subsequent releases.

## Suggested v0.1.0 release notes

- Initial public release with one Python operation, `overlap_count`, implemented
  as a Rust Polars expression plugin. It works with eager and lazy frames and
  within groups, preserving input row order.
- Counts other overlapping half-open intervals. Empty intervals receive zero;
  duplicate non-empty intervals count each other. Endpoints must have matching,
  non-null signed or unsigned 8-, 16-, 32-, or 64-bit integer dtypes; see the
  [API reference](api.md).
- Includes a Polars-independent Rust core. The project is early-stage, and its
  API and supported Polars versions may change.

The existing tests exercise the documented semantics and input errors. Python
and Polars version requirements are declared in
[pyproject.toml](https://github.com/jplauri/polars-intervals/blob/master/pyproject.toml).
Describe platform support in the release only after the corresponding artifact
checks pass; a configured build matrix is not a completed validation result.
