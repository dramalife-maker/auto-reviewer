#!/usr/bin/env python3
"""Unit tests for triage-mrs.py with mocked glab JSON fixtures."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

_SCRIPT = Path(__file__).with_name("triage-mrs.py")
_SPEC = importlib.util.spec_from_file_location("triage_mrs", _SCRIPT)
assert _SPEC and _SPEC.loader
triage_mrs = importlib.util.module_from_spec(_SPEC)
sys.modules["triage_mrs"] = triage_mrs
_SPEC.loader.exec_module(triage_mrs)

apply_label_gates = triage_mrs.apply_label_gates
find_latest_ai_note = triage_mrs.find_latest_ai_note
has_activity_since = triage_mrs.has_activity_since
load_manifest = triage_mrs.load_manifest
main = triage_mrs.main
parse_timestamp = triage_mrs.parse_timestamp
triage_all = triage_mrs.triage_all
triage_mr = triage_mrs.triage_mr


def _mr_summary(
    iid: int,
    *,
    title: str = "feat: example",
    source_branch: str = "feature/example",
    draft: bool = False,
    work_in_progress: bool = False,
    labels: list[str] | None = None,
    author_email: str = "alice@co.com",
) -> dict[str, Any]:
    return {
        "iid": iid,
        "title": title,
        "source_branch": source_branch,
        "draft": draft,
        "work_in_progress": work_in_progress,
        "labels": labels or [],
        "author": {"public_email": author_email, "username": "alice"},
    }


def _mr_detail(
    iid: int,
    *,
    notes: list[dict[str, Any]] | None = None,
    commits: list[dict[str, Any]] | None = None,
    updated_at: str | None = None,
) -> dict[str, Any]:
    detail: dict[str, Any] = {"iid": iid, "notes": notes or []}
    if commits is not None:
        detail["commits"] = commits
    if updated_at is not None:
        detail["updated_at"] = updated_at
    return detail


class TriageMrsTests(unittest.TestCase):
    def test_load_manifest_requires_mr_poll(self) -> None:
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False, encoding="utf-8") as handle:
            json.dump({"mode": "weekly_batch"}, handle)
            path = Path(handle.name)
        try:
            with self.assertRaises(ValueError):
                load_manifest(path)
        finally:
            path.unlink(missing_ok=True)

    def test_gitlab_draft_is_skipped(self) -> None:
        summary = _mr_summary(9, draft=True)
        detail = _mr_detail(9)
        eligible, skipped = triage_mr(summary, detail, ["wip"], None)
        self.assertIsNone(eligible)
        self.assertEqual(skipped, {"mr_iid": 9, "skip_reason": "gitlab_draft"})

    def test_work_in_progress_is_skipped(self) -> None:
        summary = _mr_summary(9, work_in_progress=True)
        detail = _mr_detail(9)
        eligible, skipped = triage_mr(summary, detail, [], None)
        self.assertIsNone(eligible)
        self.assertEqual(skipped["skip_reason"], "gitlab_draft")

    def test_skip_label_wip(self) -> None:
        summary = _mr_summary(11, labels=["WIP", "feature"])
        detail = _mr_detail(11)
        eligible, skipped = triage_mr(summary, detail, ["wip"], None)
        self.assertIsNone(eligible)
        self.assertEqual(skipped, {"mr_iid": 11, "skip_reason": "label:wip"})

    def test_missing_required_label(self) -> None:
        summary = _mr_summary(15, labels=["feature"])
        detail = _mr_detail(15)
        eligible, skipped = triage_mr(summary, detail, [], "ready-for-review")
        self.assertIsNone(eligible)
        self.assertEqual(
            skipped,
            {"mr_iid": 15, "skip_reason": "missing_required_label:ready-for-review"},
        )

    def test_round_one_without_ai_note(self) -> None:
        summary = _mr_summary(42, title="feat: add cache", source_branch="feature/cache")
        detail = _mr_detail(42, notes=[{"body": "Please review", "created_at": "2026-07-08T10:00:00Z"}])
        eligible, skipped = triage_mr(summary, detail, ["wip"], None)
        self.assertIsNone(skipped)
        self.assertEqual(
            eligible,
            {
                "mr_iid": 42,
                "mr_title": "feat: add cache",
                "source_branch": "feature/cache",
                "author_identity": "alice@co.com",
                "review_round": 1,
            },
        )

    def test_round_two_with_activity(self) -> None:
        summary = _mr_summary(55, title="fix: cache race", source_branch="feature/cache")
        detail = _mr_detail(
            55,
            notes=[
                {
                    "body": "Summary\n\nBy: AI Agent",
                    "created_at": "2026-07-08T08:00:00Z",
                },
                {
                    "body": "Pushed new commits",
                    "created_at": "2026-07-08T12:00:00Z",
                },
            ],
            commits=[{"committed_date": "2026-07-08T11:30:00Z", "title": "fix race"}],
        )
        eligible, skipped = triage_mr(summary, detail, [], None)
        self.assertIsNone(skipped)
        self.assertEqual(eligible["review_round"], 2)

    def test_skip_no_activity_since_ai_note(self) -> None:
        summary = _mr_summary(7)
        detail = _mr_detail(
            7,
            notes=[
                {
                    "body": "Round 1 review\n\nBy: AI Agent",
                    "created_at": "2026-07-08T08:00:00Z",
                }
            ],
            commits=[{"committed_date": "2026-07-08T07:00:00Z", "title": "older commit"}],
            updated_at="2026-07-08T08:30:00Z",
        )
        eligible, skipped = triage_mr(summary, detail, [], None)
        self.assertIsNone(eligible)
        self.assertEqual(
            skipped,
            {"mr_iid": 7, "skip_reason": "no_new_activity_since_ai_note"},
        )

    def test_apply_label_gates_case_insensitive(self) -> None:
        reason = apply_label_gates({"labels": ["Do-Not-Review"]}, ["do-not-review"], None)
        self.assertEqual(reason, "label:do-not-review")

    def test_find_latest_ai_note(self) -> None:
        notes = [
            {"body": "old\nBy: AI Agent", "created_at": "2026-07-07T08:00:00Z"},
            {"body": "newer\nBy: AI Agent", "created_at": "2026-07-08T08:00:00Z"},
        ]
        latest = find_latest_ai_note(notes)
        self.assertIsNotNone(latest)
        assert latest is not None
        self.assertIn("newer", latest["body"])
        self.assertEqual(parse_timestamp(latest["created_at"]).isoformat(), "2026-07-08T08:00:00+00:00")

    def test_has_activity_since_uses_commits(self) -> None:
        since = parse_timestamp("2026-07-08T08:00:00Z")
        assert since is not None
        detail = _mr_detail(
            1,
            commits=[{"committed_date": "2026-07-08T09:00:00Z"}],
        )
        self.assertTrue(has_activity_since(detail, since))

    def test_triage_all_integration_with_mock_glab(self) -> None:
        fixtures: dict[str, Any] = {
            "list": [
                _mr_summary(7, labels=["ready-for-review"]),
                _mr_summary(9, draft=True),
                _mr_summary(11, labels=["wip"]),
                _mr_summary(15, labels=["feature"]),
                _mr_summary(42, title="feat: add cache", source_branch="feature/cache", labels=["ready-for-review"]),
                _mr_summary(55, title="fix: cache race", source_branch="feature/cache", labels=["ready-for-review"]),
            ],
            "views": {
                7: _mr_detail(
                    7,
                    notes=[
                        {
                            "body": "Round 1\n\nBy: AI Agent",
                            "created_at": "2026-07-08T08:00:00Z",
                        }
                    ],
                ),
                9: _mr_detail(9),
                11: _mr_detail(11),
                15: _mr_detail(15),
                42: _mr_detail(42),
                55: _mr_detail(
                    55,
                    notes=[
                        {
                            "body": "Prior review\n\nBy: AI Agent",
                            "created_at": "2026-07-08T08:00:00Z",
                        }
                    ],
                    commits=[{"committed_date": "2026-07-08T10:00:00Z"}],
                ),
            },
        }

        def fake_runner(command: list[str], _cwd: Path) -> str:
            if command[1:4] == ["mr", "list", "-F"]:
                return json.dumps(fixtures["list"])
            if command[1:3] == ["mr", "view"]:
                iid = int(command[3])
                return json.dumps(fixtures["views"][iid])
            raise AssertionError(f"unexpected command: {command}")

        manifest = {
            "mode": "mr_poll",
            "repo_path": ".",
            "mr_review_skip_labels": ["wip"],
            "mr_review_require_label": "ready-for-review",
        }
        output = triage_all(manifest, "glab", Path("."), runner=fake_runner)

        eligible_iids = {item["mr_iid"] for item in output["eligible"]}
        skipped = {item["mr_iid"]: item["skip_reason"] for item in output["skipped"]}

        self.assertEqual(eligible_iids, {42, 55})
        self.assertEqual(output["eligible"][0]["review_round"], 1)
        self.assertEqual(output["eligible"][1]["review_round"], 2)
        self.assertEqual(skipped[7], "no_new_activity_since_ai_note")
        self.assertEqual(skipped[9], "gitlab_draft")
        self.assertEqual(skipped[11], "label:wip")
        self.assertEqual(skipped[15], "missing_required_label:ready-for-review")
        self.assertIn("generated_at", output)

    def test_main_writes_eligible_mrs_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            manifest_path = root / "manifest.json"
            manifest_path.write_text(
                json.dumps(
                    {
                        "mode": "mr_poll",
                        "repo_path": str(root),
                        "mr_review_skip_labels": [],
                    }
                ),
                encoding="utf-8",
            )

            fixtures = {
                "list": [_mr_summary(42)],
                "views": {42: _mr_detail(42)},
            }

            def fake_runner(command: list[str], _cwd: Path) -> str:
                if command[1:4] == ["mr", "list", "-F"]:
                    return json.dumps(fixtures["list"])
                if command[1:3] == ["mr", "view"]:
                    return json.dumps(fixtures["views"][int(command[3])])
                raise AssertionError(command)

            original_triage_all = triage_mrs.triage_all

            def patched_triage_all(manifest, glab_cmd, cwd, runner=None):
                return original_triage_all(manifest, glab_cmd, cwd, runner=fake_runner)

            triage_mrs.triage_all = patched_triage_all
            try:
                exit_code = main(["--manifest", str(manifest_path), "--glab-cmd", "glab"])
            finally:
                triage_mrs.triage_all = original_triage_all

            self.assertEqual(exit_code, 0)
            output_path = root / "eligible_mrs.json"
            self.assertTrue(output_path.exists())
            payload = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(len(payload["eligible"]), 1)
            self.assertEqual(payload["eligible"][0]["mr_iid"], 42)


if __name__ == "__main__":
    unittest.main()
