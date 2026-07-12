#!/usr/bin/env bash
set -euo pipefail
exec python3 "$(dirname "$0")/write_draft_then_hang.py"
