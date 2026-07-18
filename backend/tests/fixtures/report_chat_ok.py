#!/usr/bin/env python3
"""Successful person report-chat agent-turn stub for integration tests."""
from __future__ import annotations

import os
import sys


def main() -> int:
    summary_path = os.environ.get("REPORT_CHAT_SUMMARY_PATH")
    summary_body = os.environ.get("REPORT_CHAT_SUMMARY_BODY")
    if summary_path and summary_body is not None:
        parent = os.path.dirname(summary_path)
        if parent:
            os.makedirs(parent, exist_ok=True)
        with open(summary_path, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(summary_body)
            if summary_body and not summary_body.endswith("\n"):
                fh.write("\n")

    session_id = os.environ.get("REPORT_CHAT_SESSION_ID", "report-sess-1")
    reply = os.environ.get("REPORT_CHAT_REPLY", "updated the weekly summary")
    sys.stdout.write(
        f'{{"type":"system","subtype":"init","session_id":"{session_id}"}}\n'
    )
    sys.stdout.write(f'{{"type":"assistant","text":"{reply}"}}\n')
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
