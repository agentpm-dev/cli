"""Local deterministic model/capture fixture. Never contacts PostHog or a real model."""

import argparse
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=18140)
    parser.add_argument("--events", type=Path, required=True)
    parser.add_argument("--analytics-delay", type=float, default=0)
    parser.add_argument("--model-delay", type=float, default=0.5)
    args = parser.parse_args()
    args.events.parent.mkdir(parents=True, exist_ok=True)
    lock = threading.Lock()

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, _format, *_args):
            pass

        def do_POST(self):  # pylint: disable=invalid-name
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            if self.path == "/analytics/cli":
                with lock:
                    with args.events.open("a", encoding="utf-8") as output:
                        output.write(json.dumps(body, sort_keys=True) + "\n")
                    print(json.dumps(body, sort_keys=True), flush=True)
                time.sleep(args.analytics_delay)
                self.send_response(204)
                self.end_headers()
                return
            if self.path != "/model":
                self.send_error(404)
                return
            time.sleep(args.model_delay)
            response = {
                "choices": [
                    {
                        "message": {
                            "role": "assistant",
                            "content": None,
                            "tool_calls": [
                                {
                                    "id": "fixture-call",
                                    "type": "function",
                                    "function": {
                                        "name": "phase_complete",
                                        "arguments": json.dumps(
                                            {
                                                "outcome": "complete",
                                                "output": {"answer": "fixture-ok"},
                                            }
                                        ),
                                    },
                                }
                            ],
                        },
                        "finish_reason": "tool_calls",
                    }
                ]
            }
            encoded = json.dumps(response).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(encoded)))
            self.end_headers()
            try:
                self.wfile.write(encoded)
            except (BrokenPipeError, ConnectionResetError):
                pass

    print(f"Local fixture listening on 127.0.0.1:{args.port}", flush=True)
    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
