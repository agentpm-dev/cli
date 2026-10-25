"""Scripted local model: request the real Tool and verify its observed result."""

import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import threading


def total_in(value):
    if isinstance(value, dict):
        if value.get("total") == 9.0:
            return True
        return any(total_in(child) for child in value.values())
    if isinstance(value, list):
        return any(total_in(child) for child in value)
    return False


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=18160)
    parser.add_argument("--events", type=Path, required=True)
    args = parser.parse_args()
    args.events.parent.mkdir(parents=True, exist_ok=True)
    lock = threading.Lock()

    def record(event):
        with lock:
            with args.events.open("a", encoding="utf-8") as output:
                output.write(json.dumps(event, sort_keys=True) + "\n")
            print(json.dumps(event, sort_keys=True), flush=True)

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, _format, *_args):
            pass

        def respond(self, status, value):
            encoded = json.dumps(value).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)

        def do_GET(self):
            if self.path == "/health":
                self.respond(200, {"fixture": "m11c", "ok": True})
            else:
                self.send_error(404)

        def do_POST(self):
            if self.path != "/model":
                self.send_error(404)
                return
            try:
                request = json.loads(
                    self.rfile.read(int(self.headers["Content-Length"]))
                )
                results = [
                    message
                    for message in request.get("messages", [])
                    if message.get("role") == "tool"
                    and message.get("tool_call_id") == "m11c-order-call"
                ]
                if results:
                    value = json.loads(results[-1]["content"])
                    if not total_in(value) or value.get("ok") is False:
                        raise ValueError("The real Tool did not return total 9.0")
                    function = "phase_complete"
                    arguments = {
                        "outcome": "complete",
                        "output": {"total": 9.0, "verified_tool_result": True},
                    }
                    call_id = "m11c-complete-call"
                    record({"event": "tool_result_observed", "total": 9.0})
                else:
                    functions = [
                        tool["function"]["name"]
                        for tool in request.get("tools", [])
                        if tool["function"]["name"].startswith("agentpm_tool_")
                    ]
                    if len(functions) != 1:
                        raise ValueError("Expected exactly one available AgentPM Tool")
                    function = functions[0]
                    arguments = {"arguments": {"quantity": 2, "unit_price": 4.5}}
                    call_id = "m11c-order-call"
                    record({"event": "tool_requested", "alias": function})
                self.respond(
                    200,
                    {
                        "choices": [
                            {
                                "message": {
                                    "role": "assistant",
                                    "content": None,
                                    "tool_calls": [
                                        {
                                            "id": call_id,
                                            "type": "function",
                                            "function": {
                                                "name": function,
                                                "arguments": json.dumps(arguments),
                                            },
                                        }
                                    ],
                                },
                                "finish_reason": "tool_calls",
                            }
                        ]
                    },
                )
            except (KeyError, TypeError, ValueError) as error:
                record({"event": "fixture_error", "message": str(error)})
                self.respond(422, {"error": str(error)})

    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(
        f"Local Tool-call fixture: http://127.0.0.1:{server.server_port}/model",
        flush=True,
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
