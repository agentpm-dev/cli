"""Development-only signing smoke fixture; no dependencies or external services."""

import json

print(json.dumps({"ok": True}, separators=(",", ":")))
