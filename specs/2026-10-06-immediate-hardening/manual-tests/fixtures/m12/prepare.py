"""Create disposable manual-test inputs; never execute AgentPM or publish."""

import base64
import hashlib
import json
import shutil
import sys
from pathlib import Path

PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a"
    "X1sAAAAASUVORK5CYII="
)
GIF = b"GIF89a\x01\x00\x01\x00"
TOOL = 'import json\nprint(json.dumps({"ok": True}))\n'


def write(root, name, content):
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content.encode() if isinstance(content, str) else content)


def tool(cases, name, files, runtime="python", entrypoint="tool.py"):
    root = cases / name
    root.mkdir()
    manifest = {
        "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/main/schemas/agentpm.manifest.schema.json",
        "kind": "tool",
        "name": "m12-" + name,
        "version": "0.1.0",
        "description": "Disposable payload compatibility manual-test fixture.",
        "files": files,
        "entrypoint": {
            "command": "python3" if runtime == "python" else "node",
            "args": [entrypoint],
        },
        "runtime": {
            "type": runtime,
            "version": "3.11" if runtime == "python" else "20",
        },
        "readme": "README.md",
        "license": {"spdx": "MIT"},
        "inputs": {"type": "object"},
        "outputs": {"type": "object"},
    }
    write(root, "agent.json", json.dumps(manifest, indent=2) + "\n")
    write(root, entrypoint, TOOL if runtime == "python" else "console.log('{}');\n")
    write(root, "README.md", "# Disposable payload fixture\n")
    # A dependency-free directory case needs no author lock. An incidental
    # .lock file is unknown payload data and would obscure the collection check.
    if "." not in files:
        write(
            root,
            "agent.lock",
            json.dumps(
                {
                    "lockfile_version": 2,
                    "generated": "2026-10-24T00:00:00Z",
                    "packages": {},
                    "roots": {},
                }
            )
            + "\n",
        )
    return root


def incidental(root):
    for name, content in {
        "keep.json": "{}\n",
        ".env": "FIXTURE_ONLY=not-a-real-secret\n",
        ".env.local": "FIXTURE_ONLY=not-a-real-secret\n",
        ".env.sample": "FIXTURE_ONLY=sample\n",
        ".env.template": "FIXTURE_ONLY=template\n",
        ".env.dist": "FIXTURE_ONLY=dist\n",
        ".env.example": "FIXTURE_ONLY=example\n",
        ".env.example.local": "FIXTURE_ONLY=local\n",
        ".git/config.json": "{}\n",
        ".DS_Store": b"OS metadata fixture",
        "Thumbs.db": b"OS metadata fixture",
        "desktop.ini": "OS metadata fixture\n",
        "mypkg/source.py": "VALUE = 1\n",
        "mypkg/__pycache__/cached.pyc": b"not real bytecode",
        "mypkg/__pycache__/.env.nested": "FIXTURE_ONLY=nested\n",
        "mypkg/__pycache__/.git/config.json": "{}\n",
        ".venv/vendor/example.whl": b"must not be inspected incidentally",
        "venv/vendor/bad.png": b"must not be inspected incidentally",
        "site-packages/package/module.py": "VALUE = 1\n",
        "dist-packages/package/module.py": "VALUE = 1\n",
        "dev-python/PYVENV.CFG": "fixture marker; not a real environment\n",
        "dev-python/vendor/example.whl": b"must not be inspected incidentally",
    }.items():
        write(root, name, content)


def main():
    root, examples = (Path(arg).resolve() for arg in sys.argv[1:])
    cases = root / "cases"
    cases.mkdir()  # Refuse to overwrite a previous walkthrough.
    (root / "logs").mkdir()
    (root / "inputs").mkdir()
    write(root, "inputs/wrong-format.gif", GIF)
    write(root, "outside.json", '{"outside":true}\n')

    portable = tool(
        cases,
        "portable",
        ["tool.py", "README.md", "assets", "templates", "inputs", "sample.egg-info"],
    )
    for name, content in {
        "assets/data.JSON": "{}\n",
        "assets/icon.PNG": PNG,
        "templates/report.j2": "Hello {{ name }}\n",
        "inputs/requirements.in": "not-authoritative==1\n",
        "inputs/MANIFEST.in": "include tool.py\n",
        "sample.egg-info/PKG-INFO": "Metadata-Version: 2.1\nName: fixture\n",
    }.items():
        write(portable, name, content)
    # A real dependency declaration is intentionally absent: requirements.in is data.
    (portable / "tool.py").chmod(0o755)

    collection = tool(cases, "collection", ["."])
    incidental(collection)
    overrides = tool(
        cases, "overrides", [".", ".env", ".git/config.json", "mypkg/__pycache__"]
    )
    incidental(overrides)
    reverse = cases / "overrides-reversed"
    shutil.copytree(overrides, reverse)
    manifest = json.loads((reverse / "agent.json").read_text())
    manifest["files"].reverse()
    write(reverse, "agent.json", json.dumps(manifest, indent=2) + "\n")

    explicit = tool(cases, "explicit-wheel", [".", ".venv"])
    incidental(explicit)
    missing = tool(cases, "missing", ["tool.py", ".env.missing"])
    assert not (missing / ".env.missing").exists()

    boundaries = tool(cases, "boundaries", ["."])
    for name, content in {
        "_vendor/helper.py": "VALUE = 1\n",
        "source.egg-info/PKG-INFO": "Name: fixture\n",
        "installed.dist-info/METADATA": "Name: fixture\n",
        "Makefile": "all:\n\t@echo fixture\n",
        "Dockerfile": "FROM scratch\n",
        "CODEOWNERS": "* fixture\n",
        ".gitignore": "keep.json\n",
        ".editorconfig": "root = true\n",
        ".dockerignore": "keep.json\n",
        "keep.json": "{}\n",
    }.items():
        write(boundaries, name, content)

    required = tool(
        cases, "required-entrypoint", ["."], entrypoint="__pycache__/tool.py"
    )
    write(required, "__pycache__/other.pyc", b"unrequested bytecode fixture")

    native = tool(cases, "native", ["tool.py", "_vendor"])
    for name, content in {
        "_vendor/libhelper.so.1.2.3": b"native suffix fixture",
        "_vendor/helper.so.notes.json": b"{}\n",
        "_vendor/renamed.py": b"\x7fELFrenamed-native-signature-fixture",
    }.items():
        write(native, name, content)

    unknown = tool(cases, "unknown", ["tool.py", "assets"])
    for name in ["a.ts", "z.rtf", "image.tiff", "settings.pth"]:
        write(unknown, "assets/" + name, "plain fixture text\n")

    permissions = tool(cases, "permissions", ["tool.py", "data.csv"])
    write(permissions, "data.csv", "value\n1\n")
    (permissions / "data.csv").chmod(0o755)
    (permissions / "tool.py").chmod(0o755)

    linked = tool(cases, "symlink", ["tool.py", "data-link.json"])
    (linked / "data-link.json").symlink_to(root / "outside.json")

    mismatch = tool(cases, "mismatch", ["tool.py", "icon.png"])
    write(mismatch, "icon.png", PNG)

    node = tool(cases, "node", ["."], runtime="node", entrypoint="tool.js")
    write(node, ".env", "FIXTURE_ONLY=node\n")
    write(node, ".git/config.json", "{}\n")
    write(node, ".venv/data.txt", "fixture environment content\n")

    # Copy reviewed author inputs, never an example's .venv/build/cache directories.
    for name in ["validate-order", "summarize-text"]:
        source = examples / "tools-python" / name
        manifest = json.loads((source / "agent.json").read_text())
        destination = cases / ("example-" + name)
        destination.mkdir()
        for member in ["agent.json", "agent.lock", *manifest["files"]]:
            path = source / member
            target = destination / member
            target.parent.mkdir(parents=True, exist_ok=True)
            if path.is_dir():
                shutil.copytree(path, target, dirs_exist_ok=True)
            else:
                shutil.copy2(path, target)

    snapshot = {}
    for directory in sorted(cases.iterdir()):
        for name in ["agent.json", "agent.lock"]:
            path = directory / name
            snapshot[str(path.relative_to(root))] = (
                hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else None
            )
    write(root, "authored-before.json", json.dumps(snapshot, indent=2) + "\n")
    print("Prepared disposable cases:", cases)
    print("No CLI commands, dependency downloads or publication were performed.")


if __name__ == "__main__":
    main()
