"""A deliberately legacy, locally staged Tool; no dependency declarations."""

import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent / "_vendor"))
from legacy_helper import VALUE

print(json.dumps({"legacy": VALUE, "prefix": sys.prefix}))
