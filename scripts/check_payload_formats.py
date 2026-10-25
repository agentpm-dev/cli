"""Generate/check payload inventory documentation from the embedded Rust input."""

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / "crates/agentpm-sdk/contracts/tool-payload-formats-v1.json"
SPEC = ROOT / "specs/2026-10-06-immediate-hardening/spec.md"


def quoted(values):
    return ", ".join(f"`{value}`" for value in values)


def sections(policy):
    if policy["type"] != "agentpm.tool-payload-formats.v1":
        raise ValueError("unsupported payload format inventory type/version")
    text = ["| Group | Candidate suffixes |", "| --- | --- |"]
    for group in policy["text_groups"]:
        text.append(f"| {group['name']} | {quoted(group['suffixes'])} |")
    text.extend(
        [
            "",
            "Exact text basenames: " + quoted(policy["text_basenames"]) + ".",
            "",
            "Text basename prefixes: " + quoted(policy["text_basename_prefixes"]) + ".",
            "",
            "Single-file text metadata suffixes: "
            + quoted(policy["text_metadata"]["file_suffixes"])
            + ".",
            "",
            "Scoped text metadata basenames: "
            + quoted(policy["text_metadata"]["basenames"])
            + " inside directory suffixes "
            + quoted(policy["text_metadata"]["directory_suffixes"])
            + ".",
        ]
    )
    binary = ["| Suffix | Required format recognition |", "| --- | --- |"]
    for fmt in policy["binary_formats"]:
        checks = []
        for signature in fmt["signatures"]:
            value = bytes.fromhex(signature["hex"]).hex(" ")
            checks.append(f"bytes `{value}` at offset {signature['offset']}")
        recognition = (
            f"{fmt['name']}: at least {fmt['min_header_bytes']} bytes; "
            + "; ".join(checks)
            + " (all required)"
        )
        binary.append(f"| {quoted(fmt['suffixes'])} | {recognition} |")
    state = policy["installed_state"]
    native_rules = policy["native_filenames"]
    native = [
        "| Native filename match (basename only, case-insensitive) | Values |",
        "| --- | --- |",
        "| Ends with a suffix | " + quoted(native_rules["suffixes"]) + " |",
        "| Contains a literal versioned-name fragment | "
        + quoted(native_rules["basename_contains"])
        + " |",
    ]
    rules = policy["collection"]
    collection = [
        "| Incidental traversal match, at any depth | Action |",
        "| --- | --- |",
        "| Basenames "
        + quoted(rules["any_basenames"])
        + " (file, link or directory) | Omit entry or prune directory |",
        "| Environment/state directory basenames "
        + quoted(state["directory_basenames"])
        + " | Prune directory; omit same-named links without following |",
        "| A discovered directory with a direct regular-file marker "
        + quoted(state["marker_basenames"])
        + " | Prune the whole detected environment root |",
        "| File/link basenames "
        + quoted(state["marker_basenames"])
        + " | Omit incidental marker |",
        "| OS file/link basenames "
        + quoted(rules["file_basenames"])
        + " | Omit entry |",
        "| File/link basename "
        + quoted([rules["dotenv_basename"]])
        + " or prefix "
        + quoted([rules["dotenv_prefix"]])
        + " | Omit except exact basenames "
        + quoted(rules["dotenv_exceptions"])
        + " |",
        "",
        "Classification-only metadata directory suffixes: "
        + quoted(state["metadata_directory_suffixes"])
        + "; not automatic traversal exclusions.",
    ]
    return {
        "text": "\n".join(text),
        "native": "\n".join(native),
        "binary": "\n".join(binary),
        "collection": "\n".join(collection),
    }


def synchronize(document, blocks):
    for name, content in blocks.items():
        markers = [
            (
                f"<!-- BEGIN tool-payload-formats:{name} -->",
                f"<!-- END tool-payload-formats:{name} -->",
            ),
            (
                f"{{/* BEGIN tool-payload-formats:{name} */}}",
                f"{{/* END tool-payload-formats:{name} */}}",
            ),
        ]
        present = [(begin, end) for begin, end in markers if begin in document]
        if len(present) != 1:
            raise ValueError(f"expected one generated {name} block")
        begin, end = present[0]
        if document.count(begin) != 1 or document.count(end) != 1:
            raise ValueError(f"expected one generated {name} block")
        before, rest = document.split(begin)
        _, after = rest.split(end)
        document = before + begin + "\n" + content + "\n" + end + after
    return document


def check_documents(policy, paths, write=False):
    blocks = sections(policy)
    stale = []
    for path in paths:
        original = path.read_text()
        expected = synchronize(original, blocks)
        if original != expected:
            if write:
                path.write_text(expected)
            else:
                stale.append(str(path))
    if stale:
        raise ValueError(
            "payload format documentation differs from the policy inventory: "
            + ", ".join(stale)
            + "; run python3 scripts/check_payload_formats.py --write"
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="read-only check (default)")
    mode.add_argument("--write", action="store_true", help="refresh generated sections")
    parser.add_argument(
        "--docs",
        type=Path,
        action="append",
        help="documents to check instead of defaults",
    )
    args = parser.parse_args()
    # Product docs link the authoritative inventory rather than repeat an
    # exhaustive copy in another repository. Explicit --docs paths must exist.
    paths = args.docs if args.docs else [SPEC]
    try:
        check_documents(json.loads(POLICY.read_text()), paths, args.write)
    except (ValueError, KeyError, TypeError, OSError) as error:
        parser.exit(1, f"Payload format check failed: {error}\n")
    print("Payload format documentation matches the policy inventory.")


if __name__ == "__main__":
    main()
