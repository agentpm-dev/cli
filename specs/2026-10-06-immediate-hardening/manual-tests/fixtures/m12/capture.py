"""Loopback-only publish capture. No Registry, S3, auth or secret-header logging."""

import hashlib
import io
import json
import sys
import tarfile
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path


def serve(root):
    root.mkdir()

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def reply(self, status, value):
            body = json.dumps(value).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            self.reply(200 if self.path == "/health" else 404, {"fixture": True})

        def receive(self):
            with (root / "events.jsonl").open("a") as stream:
                stream.write(
                    json.dumps({"method": self.command, "path": self.path}) + "\n"
                )
            size = int(self.headers.get("Content-Length", "0"))
            if not 0 <= size <= 16 * 1024 * 1024:
                raise ValueError("fixture body exceeds 16 MiB")
            body = self.rfile.read(size)
            if len(body) != size:
                raise ValueError("incomplete fixture request")
            return body

        def do_POST(self):
            try:
                value = json.loads(self.receive())
                if self.path == "/v1/tools/publish/init":
                    upload_id = str(len(list(root.glob("upload-*"))) + 1)
                    directory = root / ("upload-" + upload_id)
                    directory.mkdir()
                    (directory / "metadata.json").write_text(
                        json.dumps(value, indent=2) + "\n"
                    )
                    self.reply(
                        200,
                        {
                            "upload_id": upload_id,
                            "put_url": base + "/object/" + upload_id,
                            "headers": {},
                            "final_key": "fixture-only.tar.gz",
                            "expires_in": 900,
                        },
                    )
                elif self.path == "/v1/tools/publish/finalize":
                    upload_id = value["upload_id"]
                    directory = root / ("upload-" + str(int(upload_id)))
                    if not (directory / "object.tar.gz").is_file():
                        raise ValueError("finalize before upload")
                    metadata = json.loads((directory / "metadata.json").read_text())
                    (directory / "finalize.json").write_text(
                        json.dumps(value, indent=2) + "\n"
                    )
                    self.reply(
                        200,
                        {
                            "id": upload_id,
                            "name": metadata["manifest"]["name"],
                            "version": metadata["manifest"]["version"],
                            "url": base + "/fixture-receipt/" + upload_id,
                            "message": "Captured locally; nothing published to a Registry.",
                        },
                    )
                else:
                    self.reply(404, {"error": "unexpected fixture route"})
            except (ValueError, KeyError, OSError) as error:
                self.reply(400, {"error": str(error)})

        def do_PUT(self):
            try:
                body = self.receive()
                if not self.path.startswith("/object/"):
                    self.reply(404, {"error": "unexpected fixture route"})
                    return
                upload_id = str(int(self.path.removeprefix("/object/")))
                directory = root / ("upload-" + upload_id)
                metadata = json.loads((directory / "metadata.json").read_text())
                if len(body) != metadata["size"]:
                    raise ValueError("uploaded size does not match init")
                if hashlib.sha256(body).hexdigest() != metadata["sha256"]:
                    raise ValueError("uploaded digest does not match init")
                with tarfile.open(fileobj=io.BytesIO(body), mode="r:gz") as archive:
                    members = [
                        {
                            "name": member.name,
                            "mode": oct(member.mode),
                            "link": member.linkname,
                        }
                        for member in archive.getmembers()
                    ]
                (directory / "object.tar.gz").write_bytes(body)
                (directory / "members.json").write_text(
                    json.dumps(members, indent=2) + "\n"
                )
                self.reply(200, {"captured": True})
            except (ValueError, KeyError, OSError, tarfile.TarError) as error:
                self.reply(400, {"error": str(error)})

    server = HTTPServer(("127.0.0.1", 0), Handler)
    base = "http://127.0.0.1:" + str(server.server_port)
    (root / "events.jsonl").write_text("")
    (root / "server.json").write_text(json.dumps({"base_url": base}) + "\n")
    print("Capture listening at", base, flush=True)
    try:
        server.serve_forever()
    finally:
        server.server_close()


if __name__ == "__main__":
    serve(Path(sys.argv[1]).resolve())
