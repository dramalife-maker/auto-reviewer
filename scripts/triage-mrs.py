#!/usr/bin/env python3
"""Enumerate open MRs via glab, apply readiness gates and round/dedup logic, write eligible_mrs.json."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Sequence

AI_AGENT_MARKER = "By: AI Agent"  # keep in sync with backend `mr_reviews::AI_AGENT_MARKER`


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Triage open merge requests for MR poll scans.")
    parser.add_argument("--manifest", required=True, type=Path, help="Path to mr_poll manifest JSON")
    parser.add_argument(
        "--glab-cmd",
        default="glab",
        help="glab executable name or path (default: glab)",
    )
    return parser.parse_args(argv)


def load_manifest(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("mode") != "mr_poll":
        raise ValueError(f"manifest mode must be mr_poll, got {data.get('mode')!r}")
    return data


def resolve_cwd(manifest: dict[str, Any]) -> Path:
    raw = manifest.get("resident_worktree") or manifest.get("repo_path")
    if not raw:
        raise ValueError("manifest must include repo_path or resident_worktree")
    return Path(raw)


def resolve_output_path(manifest: dict[str, Any], manifest_path: Path) -> Path:
    raw = manifest.get("eligible_mrs_path")
    if raw:
        return Path(raw)
    return manifest_path.parent / "eligible_mrs.json"


def run_glab(
    glab_cmd: str,
    args: list[str],
    cwd: Path,
    runner: Callable[[list[str], Path], str] | None = None,
) -> Any:
    command = [glab_cmd, *args]
    if runner is not None:
        stdout = runner(command, cwd)
    else:
        result = subprocess.run(
            command,
            cwd=cwd,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            stderr = result.stderr.strip() or result.stdout.strip()
            raise RuntimeError(f"glab failed ({' '.join(command)}): {stderr}")
        stdout = result.stdout
    try:
        return json.loads(stdout)
    except json.JSONDecodeError as err:
        raise RuntimeError(f"glab returned invalid JSON for {' '.join(command)}: {err}") from err


def list_open_mrs(
    glab_cmd: str,
    cwd: Path,
    runner: Callable[[list[str], Path], str] | None = None,
) -> list[dict[str, Any]]:
    data = run_glab(glab_cmd, ["mr", "list", "-F", "json"], cwd, runner=runner)
    if isinstance(data, list):
        return data
    if isinstance(data, dict):
        for key in ("data", "merge_requests", "items"):
            value = data.get(key)
            if isinstance(value, list):
                return value
    raise RuntimeError("unexpected glab mr list JSON shape")


def view_mr(
    glab_cmd: str,
    cwd: Path,
    mr_iid: int,
    runner: Callable[[list[str], Path], str] | None = None,
) -> dict[str, Any]:
    data = run_glab(
        glab_cmd,
        ["mr", "view", str(mr_iid), "-F", "json", "-c"],
        cwd,
        runner=runner,
    )
    if isinstance(data, dict):
        return data
    raise RuntimeError(f"unexpected glab mr view JSON shape for iid {mr_iid}")


def normalize_label(label: Any) -> str:
    if isinstance(label, str):
        return label.strip().lower()
    if isinstance(label, dict):
        for key in ("name", "title", "label"):
            value = label.get(key)
            if isinstance(value, str):
                return value.strip().lower()
    return str(label).strip().lower()


def mr_labels(mr: dict[str, Any]) -> list[str]:
    raw = mr.get("labels") or []
    if isinstance(raw, list):
        return [normalize_label(item) for item in raw]
    return []


def is_draft_mr(mr: dict[str, Any]) -> bool:
    return bool(mr.get("draft") or mr.get("work_in_progress"))


def author_identity(mr: dict[str, Any]) -> str:
    author = mr.get("author") or {}
    if isinstance(author, dict):
        for key in ("public_email", "email", "username", "name"):
            value = author.get(key)
            if isinstance(value, str) and value.strip():
                return value.strip()
    for key in ("author_email", "author_username"):
        value = mr.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
    return "unknown"


def parse_timestamp(value: Any) -> datetime | None:
    if not value or not isinstance(value, str):
        return None
    text = value.strip()
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        parsed = datetime.fromisoformat(text)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        return parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def note_body(note: dict[str, Any]) -> str:
    for key in ("body", "note", "message", "content"):
        value = note.get(key)
        if isinstance(value, str):
            return value
    return ""


def extract_notes(mr: dict[str, Any]) -> list[dict[str, Any]]:
    notes: list[dict[str, Any]] = []
    for key in ("notes", "comments", "user_notes", "system_notes", "discussions"):
        value = mr.get(key)
        if isinstance(value, list):
            for item in value:
                if isinstance(item, dict):
                    notes.append(item)
        elif isinstance(value, dict):
            for nested_key in ("notes", "comments", "items"):
                nested = value.get(nested_key)
                if isinstance(nested, list):
                    for item in nested:
                        if isinstance(item, dict):
                            notes.append(item)
    return notes


def extract_commits(mr: dict[str, Any]) -> list[dict[str, Any]]:
    commits: list[dict[str, Any]] = []
    for key in ("commits", "commit_list"):
        value = mr.get(key)
        if isinstance(value, list):
            commits.extend(item for item in value if isinstance(item, dict))
    return commits


def commit_timestamp(commit: dict[str, Any]) -> datetime | None:
    for key in ("committed_date", "created_at", "authored_date"):
        parsed = parse_timestamp(commit.get(key))
        if parsed is not None:
            return parsed
    return None


def note_timestamp(note: dict[str, Any]) -> datetime | None:
    for key in ("created_at", "updated_at"):
        parsed = parse_timestamp(note.get(key))
        if parsed is not None:
            return parsed
    return None


def find_latest_ai_note(notes: list[dict[str, Any]]) -> dict[str, Any] | None:
    latest: dict[str, Any] | None = None
    latest_time: datetime | None = None
    for note in notes:
        if AI_AGENT_MARKER not in note_body(note):
            continue
        note_time = note_timestamp(note)
        if note_time is None:
            if latest is None:
                latest = note
            continue
        if latest_time is None or note_time > latest_time:
            latest = note
            latest_time = note_time
    return latest


def has_activity_since(mr: dict[str, Any], since: datetime) -> bool:
    for commit in extract_commits(mr):
        commit_time = commit_timestamp(commit)
        if commit_time is not None and commit_time > since:
            return True

    for note in extract_notes(mr):
        note_time = note_timestamp(note)
        if note_time is None or note_time <= since:
            continue
        return True

    updated_at = parse_timestamp(mr.get("updated_at"))
    if updated_at is not None and updated_at > since:
        last_activity = parse_timestamp(mr.get("last_activity_at"))
        if last_activity is not None and last_activity > since:
            return True

    return False


def apply_label_gates(
    mr: dict[str, Any],
    skip_labels: list[str],
    require_label: str | None,
) -> str | None:
    labels = mr_labels(mr)
    normalized_skip = {normalize_label(label) for label in skip_labels if label}
    for label in labels:
        if label in normalized_skip:
            return f"label:{label}"
    if require_label:
        required = normalize_label(require_label)
        if required not in labels:
            return f"missing_required_label:{require_label}"
    return None


def triage_mr(
    mr_summary: dict[str, Any],
    mr_detail: dict[str, Any],
    skip_labels: list[str],
    require_label: str | None,
) -> tuple[dict[str, Any] | None, dict[str, Any] | None]:
    mr_iid = int(mr_summary.get("iid") or mr_detail.get("iid"))
    merged = {**mr_summary, **mr_detail, "iid": mr_iid}

    if is_draft_mr(merged):
        return None, {"mr_iid": mr_iid, "skip_reason": "gitlab_draft"}

    label_reason = apply_label_gates(merged, skip_labels, require_label)
    if label_reason:
        return None, {"mr_iid": mr_iid, "skip_reason": label_reason}

    notes = extract_notes(mr_detail)
    ai_note = find_latest_ai_note(notes)
    base = {
        "mr_iid": mr_iid,
        "mr_title": str(merged.get("title") or ""),
        "source_branch": str(merged.get("source_branch") or ""),
        "target_branch": str(merged.get("target_branch") or ""),
        "author_identity": author_identity(merged),
    }
    if ai_note is None:
        return {**base, "review_round": 1}, None

    ai_note_time = note_timestamp(ai_note)
    if ai_note_time is None:
        return {**base, "review_round": 2}, None

    if has_activity_since(mr_detail, ai_note_time):
        return {**base, "review_round": 2}, None

    return None, {"mr_iid": mr_iid, "skip_reason": "no_new_activity_since_ai_note"}


def triage_all(
    manifest: dict[str, Any],
    glab_cmd: str,
    cwd: Path,
    runner: Callable[[list[str], Path], str] | None = None,
) -> dict[str, Any]:
    skip_labels = manifest.get("mr_review_skip_labels") or []
    if not isinstance(skip_labels, list):
        raise ValueError("mr_review_skip_labels must be a JSON array")
    require_label = manifest.get("mr_review_require_label")
    if require_label is not None and not isinstance(require_label, str):
        raise ValueError("mr_review_require_label must be a string when set")

    eligible: list[dict[str, Any]] = []
    skipped: list[dict[str, Any]] = []

    for mr_summary in list_open_mrs(glab_cmd, cwd, runner=runner):
        mr_iid = mr_summary.get("iid")
        if mr_iid is None:
            continue
        mr_detail = view_mr(glab_cmd, cwd, int(mr_iid), runner=runner)
        item, skip = triage_mr(mr_summary, mr_detail, skip_labels, require_label)
        if item is not None:
            eligible.append(item)
        if skip is not None:
            skipped.append(skip)

    eligible.sort(key=lambda item: item["mr_iid"])
    skipped.sort(key=lambda item: item["mr_iid"])
    return {
        "generated_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "eligible": eligible,
        "skipped": skipped,
    }


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        manifest = load_manifest(args.manifest)
        cwd = resolve_cwd(manifest)
        if not cwd.is_dir():
            raise ValueError(f"worktree does not exist: {cwd}")

        output = triage_all(manifest, args.glab_cmd, cwd)
        output_path = resolve_output_path(manifest, args.manifest)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(json.dumps(output, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    except (ValueError, RuntimeError, OSError) as err:
        print(f"triage-mrs: {err}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
