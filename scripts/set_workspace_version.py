"""Set the workspace version in Cargo.toml.

Reads the target version from the ``VERSION`` environment variable and:

1. Bumps ``[workspace.package].version`` (every crate inherits via
   ``version = { workspace = true }``).
2. Bumps the ``version`` pins on intra-workspace path-deps in
   ``[workspace.dependencies]``. cargo requires these to match the bumped
   crate versions or resolution fails with "candidate versions found which
   didn't match".
"""

import os
import pathlib
import re
import tomllib


def main() -> None:
    version = os.environ["VERSION"]
    path = pathlib.Path("Cargo.toml")
    text = path.read_text()
    data = tomllib.loads(text)

    # 1) Bump [workspace.package].version (every crate inherits via
    #    `version = { workspace = true }`).
    new_text, count = re.subn(
        r'(\[workspace\.package\][^\[]*?\nversion\s*=\s*")[^"]*(")',
        lambda m: m.group(1) + version + m.group(2),
        text,
        count=1,
        flags=re.DOTALL,
    )
    if count != 1:
        raise SystemExit("Failed to update [workspace.package] version in Cargo.toml")

    # 2) Bump version= pins on intra-workspace path-deps in
    #    [workspace.dependencies]. cargo requires these to match
    #    the bumped crate versions or resolution fails with
    #    "candidate versions found which didn't match".
    deps = data.get("workspace", {}).get("dependencies", {})
    for name, spec in deps.items():
        if not isinstance(spec, dict):
            continue
        if not isinstance(spec.get("path"), str):
            continue
        if not spec["path"].startswith("crates/"):
            continue
        if "version" not in spec:
            continue
        pattern = (
            r'^(' + re.escape(name)
            + r'\s*=\s*\{[^}]*?version\s*=\s*")[^"]*(")'
        )
        new_text, n2 = re.subn(
            pattern,
            lambda m: m.group(1) + version + m.group(2),
            new_text,
            count=1,
            flags=re.MULTILINE,
        )
        if n2 != 1:
            raise SystemExit(
                f"Failed to update workspace dep version pin for `{name}`"
            )
        print(f"  pinned {name} -> {version}")

    path.write_text(new_text)
    print(f"Set workspace version to {version}")


if __name__ == "__main__":
    main()
