#!/usr/bin/env python3
"""Successful agent-turn stub for integration tests."""
from __future__ import annotations

import os
import sys


def main() -> int:
    draft_path = os.environ.get("AGENT_TURN_DRAFT_PATH")
    draft_body = os.environ.get("AGENT_TURN_DRAFT_BODY")
    if draft_path and draft_body is not None:
        with open(draft_path, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(draft_body)
            if draft_body and not draft_body.endswith("\n"):
                fh.write("\n")
    sys.stdout.write('{"type":"assistant","text":"because it wraps commits"}\n')
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
