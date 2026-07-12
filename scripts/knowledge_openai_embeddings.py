#!/usr/bin/env python3
import argparse
import array
import json
import os
import ssl
import sys
import urllib.error
import urllib.request
from pathlib import Path


DEFAULT_MODEL = "text-embedding-3-small"
EMBEDDINGS_URL = "https://api.openai.com/v1/embeddings"

try:
    import certifi  # type: ignore
except ImportError:
    certifi = None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate OpenAI embeddings for AgentPM Knowledge manual testing."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    chunks = subparsers.add_parser(
        "chunks-to-f32",
        help="Read knowledge/chunks.jsonl and write a little-endian float32 vectors file.",
    )
    chunks.add_argument("--chunks", required=True, help="Path to chunks.jsonl")
    chunks.add_argument("--output", required=True, help="Path to write .f32 output")
    chunks.add_argument("--model", default=DEFAULT_MODEL, help="Embedding model")

    query = subparsers.add_parser(
        "query-to-json",
        help="Embed a query string or input file and write query.json for agentpm knowledge query.",
    )
    query_group = query.add_mutually_exclusive_group(required=True)
    query_group.add_argument("--text", help="Inline query text")
    query_group.add_argument("--input-file", help="Path to a text file containing the query")
    query.add_argument("--output", required=True, help="Path to write query JSON")
    query.add_argument("--model", default=DEFAULT_MODEL, help="Embedding model")

    adapter = subparsers.add_parser(
        "adapter",
        help="Read the AgentPM embedding adapter stdin contract and write vector JSON to stdout.",
    )
    adapter.add_argument(
        "--default-model",
        default=DEFAULT_MODEL,
        help="Fallback embedding model when stdin omits embedding.model",
    )

    return parser.parse_args()


def require_api_key() -> str:
    api_key = os.environ.get("OPENAI_API_KEY")
    if not api_key:
        raise SystemExit("OPENAI_API_KEY is required")
    return api_key


def load_query_text(args: argparse.Namespace) -> str:
    if args.text is not None:
        return args.text
    return Path(args.input_file).read_text(encoding="utf-8")


def request_embeddings(texts: list[str], model: str, api_key: str) -> list[list[float]]:
    payload = json.dumps({"model": model, "input": texts}).encode("utf-8")
    ssl_context = build_ssl_context()
    request = urllib.request.Request(
        EMBEDDINGS_URL,
        data=payload,
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, context=ssl_context) as response:
            body = response.read().decode("utf-8")
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise SystemExit(f"OpenAI embeddings request failed: {exc.code} {detail}") from exc
    except ssl.SSLCertVerificationError as exc:
        raise SystemExit(build_ssl_help_message(exc)) from exc
    except urllib.error.URLError as exc:
        if isinstance(exc.reason, ssl.SSLCertVerificationError):
            raise SystemExit(build_ssl_help_message(exc.reason)) from exc
        raise SystemExit(f"OpenAI embeddings request failed: {exc.reason}") from exc

    data = json.loads(body)
    rows = data.get("data", [])
    if not rows:
        raise SystemExit("OpenAI embeddings response did not include any vectors")

    embeddings = [row["embedding"] for row in rows]
    first_dimensions = len(embeddings[0])
    if any(len(row) != first_dimensions for row in embeddings):
        raise SystemExit("OpenAI embeddings response returned inconsistent vector lengths")
    return embeddings


def write_f32(path: Path, vectors: list[list[float]]) -> None:
    values = array.array("f")
    for row in vectors:
        values.extend(row)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("wb") as handle:
        values.tofile(handle)


def build_ssl_context() -> ssl.SSLContext:
    if certifi is not None:
        return ssl.create_default_context(cafile=certifi.where())
    return ssl.create_default_context()


def build_ssl_help_message(exc: BaseException) -> str:
    return (
        f"OpenAI embeddings request failed: {exc}\n"
        "Python could not verify the TLS certificate chain.\n"
        "Try one of these:\n"
        "1. Install certifi into the Python environment running this script: "
        "`python3 -m pip install certifi`\n"
        "2. On macOS system Python, run the bundled certificate installer if available.\n"
        "3. Use a Python environment that already has an up-to-date CA bundle."
    )


def command_chunks_to_f32(args: argparse.Namespace) -> None:
    api_key = require_api_key()
    chunks_path = Path(args.chunks)
    texts: list[str] = []
    with chunks_path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            stripped = line.strip()
            if not stripped:
                continue
            record = json.loads(stripped)
            text = record.get("text")
            if not isinstance(text, str) or not text.strip():
                raise SystemExit(
                    f'{chunks_path}:{line_number} is missing a non-empty "text" field'
                )
            texts.append(text)

    if not texts:
        raise SystemExit(f"{chunks_path} did not contain any chunk rows")

    embeddings = request_embeddings(texts, args.model, api_key)
    write_f32(Path(args.output), embeddings)
    print(
        f"Wrote {len(embeddings)} vectors x {len(embeddings[0])} dimensions to {args.output}",
        file=sys.stderr,
    )


def command_query_to_json(args: argparse.Namespace) -> None:
    api_key = require_api_key()
    text = load_query_text(args)
    embeddings = request_embeddings([text], args.model, api_key)
    vector = embeddings[0]
    output = {
        "vector": vector,
        "provider": "openai",
        "model": args.model,
        "dimensions": len(vector),
    }
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(output, indent=2) + "\n", encoding="utf-8")
    print(
        f"Wrote query vector with {len(vector)} dimensions to {args.output}",
        file=sys.stderr,
    )


def command_adapter(args: argparse.Namespace) -> None:
    api_key = require_api_key()
    payload = json.load(sys.stdin)
    text = payload.get("text")
    if not isinstance(text, str) or not text.strip():
        raise SystemExit('adapter stdin payload must include non-empty "text"')
    embedding = payload.get("embedding")
    if embedding is not None and not isinstance(embedding, dict):
        raise SystemExit('adapter stdin payload "embedding" must be an object when present')
    model = (
        embedding.get("model")
        if isinstance(embedding, dict) and isinstance(embedding.get("model"), str)
        else args.default_model
    )
    embeddings = request_embeddings([text], model, api_key)
    vector = embeddings[0]
    output = {
        "vector": vector,
        "provider": "openai",
        "model": model,
        "dimensions": len(vector),
    }
    json.dump(output, sys.stdout)
    sys.stdout.write("\n")


def main() -> None:
    args = parse_args()
    if args.command == "chunks-to-f32":
        command_chunks_to_f32(args)
        return
    if args.command == "query-to-json":
        command_query_to_json(args)
        return
    if args.command == "adapter":
        command_adapter(args)
        return
    raise SystemExit(f"Unsupported command: {args.command}")


if __name__ == "__main__":
    main()
