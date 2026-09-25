"""Skip native builds only for pull requests containing documentation changes."""

import json
import os
import subprocess
from pathlib import Path

DOC_FILES = {"README.md", "CONTRIBUTING.md", "mkdocs.yml", "benchmarks/README.md"}


def requires_build(event_name, event):
    if event_name != "pull_request":
        return True

    pull = event["pull_request"]
    comparison = f"{pull['base']['sha']}...{pull['head']['sha']}"
    paths = subprocess.check_output(
        ["git", "diff", "--name-only", "--no-renames", "-z", comparison],
        text=True,
    ).split("\0")[:-1]
    return not paths or any(
        path not in DOC_FILES and not path.startswith(("docs/", "benchmarks/results/"))
        for path in paths
    )


if __name__ == "__main__":
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
    required = requires_build(os.environ["GITHUB_EVENT_NAME"], event)
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        print(f"build_required={str(required).lower()}", file=output)
    print(
        "Native builds required." if required else "Documentation-only PR: skipping native builds."
    )
