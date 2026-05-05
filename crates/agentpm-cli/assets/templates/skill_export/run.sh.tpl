#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ge 2 && "$1" == "--input-file" ]]; then
  agentpm run {{PACKAGE_REF}} --input-file "$2"
elif [[ $# -ge 1 ]]; then
  agentpm run {{PACKAGE_REF}} --input "$1"
else
  agentpm run {{PACKAGE_REF}}
fi
