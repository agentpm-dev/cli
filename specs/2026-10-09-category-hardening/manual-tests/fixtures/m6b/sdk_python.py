"""Use the checkout SDK against the real CLI; retain raw envelopes and requests."""

import json
from pathlib import Path
import sys

from model_service import turn


def main():
    sdk, cli, mode, agent = sys.argv[1:]
    sys.path.insert(0, str(Path(sdk) / "src"))
    from agentpm import HarnessClient, HarnessProtocolError

    client = HarnessClient(agentpm_path=cli, cwd=str(Path.cwd()), agent=agent or None)
    requests = []

    def model(payload):
        requests.append(payload["request"])
        return turn(len(requests))

    if mode == "run":
        client.register_model_provider("sdk-model", model)
    try:
        info = client.initialize()
        preflight = client.preflight()
        supported = client.supported_standards()
        result, blocked = None, None
        if mode == "run":
            result = client.run("Check exact versions.")
        elif mode == "blocked":
            try:
                client.run("Must not execute.")
            except HarnessProtocolError as error:
                blocked = error.code
            assert blocked == "preflight_not_ready", blocked
        client.shutdown()
        print(
            json.dumps(
                dict(
                    sdk="python",
                    initialized=info,
                    preflight=preflight,
                    supported=supported,
                    result=result,
                    blocked=blocked,
                    requests=requests,
                )
            )
        )
    finally:
        client.stop()


if __name__ == "__main__":
    main()
