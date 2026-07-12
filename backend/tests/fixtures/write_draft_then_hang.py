#!/usr/bin/env python3
"""Test fixture: write an MR draft then hang until killed by timeout."""

from __future__ import annotations

import os
import time
from pathlib import Path


def main() -> None:
    draft_path = os.environ.get("REVIEWER_TEST_DRAFT_FILE")
    if not draft_path:
        raise SystemExit("REVIEWER_TEST_DRAFT_FILE is required")

    path = Path(draft_path)
    path.parent.mkdir(parents=True, exist_ok=True)
    mr_iid = os.environ.get("REVIEWER_TEST_MR_IID", "68")
    path.write_text(
        "\n".join(
            [
                "---",
                f"mr_iid: {mr_iid}",
                'mr_title: "feat test"',
                "review_round: 1",
                "author_identity: alice@example.com",
                "---",
                "",
                "Draft body written before hang.",
                "",
            ]
        ),
        encoding="utf-8",
    )
    # Exceed typical per_project_timeout_sec used in tests (1s).
    time.sleep(10)


if __name__ == "__main__":
    main()
