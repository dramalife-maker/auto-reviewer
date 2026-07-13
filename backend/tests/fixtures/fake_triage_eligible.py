#!/usr/bin/env python3
"""Test fixture: write a single eligible MR from the manifest path."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--glab-cmd", default="glab")
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    out = Path(manifest["eligible_mrs_path"])
    out.parent.mkdir(parents=True, exist_ok=True)

    mr_iid = int(os.environ.get("REVIEWER_TEST_MR_IID", "68"))
    payload = {
        "generated_at": "2026-07-12T00:00:00Z",
        "eligible": [
            {
                "mr_iid": mr_iid,
                "mr_title": "feat test",
                "source_branch": "main",
                "target_branch": "main",
                "author_identity": "alice@example.com",
                "review_round": 1,
            }
        ],
        "skipped": [],
    }
    out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
