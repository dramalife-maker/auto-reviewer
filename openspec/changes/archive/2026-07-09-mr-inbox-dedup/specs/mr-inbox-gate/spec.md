## ADDED Requirements

### Requirement: MR scan worker skips inbox-blocked review rounds

After `triage-mrs.py` completes and the worker reads `eligible_mrs.json`, the worker MUST filter eligible entries before spawning any per-MR agent subprocess. For each eligible entry, the worker MUST look up `mr_reviews` for a row with matching `project_id`, `mr_iid`, and `review_round`. If such a row exists with `status='draft'`, the worker MUST NOT spawn an agent for that entry and MUST record skip reason `inbox_draft`. If such a row exists with `status='ignored'`, the worker MUST NOT spawn an agent and MUST record skip reason `inbox_ignored`. Rows with `status='published'` MUST NOT block spawning via this gate (GitLab note dedup remains the responsibility of triage).

Scheduled `mr_poll` runs and manual `mr-scan` requests without a force flag MUST apply this gate. The triage script MUST NOT query SQLite.

#### Scenario: Draft inbox row prevents rescan

- **WHEN** triage lists MR IID 12 at `review_round=1` for project 3 and `mr_reviews` contains `(project_id=3, mr_iid=12, review_round=1, status='draft')`
- **THEN** the worker completes the run project without spawning an agent for MR 12 and logs skip reason `inbox_draft`

#### Scenario: Ignored inbox row prevents rescan

- **WHEN** triage lists MR IID 8 at `review_round=2` for project 3 and `mr_reviews` contains `(project_id=3, mr_iid=8, review_round=2, status='ignored')`
- **THEN** the worker does not spawn an agent for MR 8 and logs skip reason `inbox_ignored`

#### Scenario: Published row does not block via inbox gate

- **WHEN** triage lists MR IID 5 at `review_round=1` for project 3 and the only matching `mr_reviews` row has `status='published'`
- **THEN** the inbox gate does not skip MR 5 solely because of that row (triage eligibility governs whether an agent runs)

### Requirement: Manual MR scan supports force bypass of inbox gate

The backend SHALL accept an optional query parameter `force` on `POST /api/projects/:id/mr-scan`. When `force` is `1` or `true` (case-insensitive), the worker MUST NOT apply the inbox-blocked filter for that run and MUST spawn agents for all triage-eligible entries as if no `draft` or `ignored` rows existed. When `force` is absent or falsy, the inbox gate MUST apply. Scheduled `mr_poll` triggers MUST NOT set `force`.

Conflict behavior (HTTP 409 when a run is already queued or running) MUST remain unchanged.

#### Scenario: Force scan reruns despite draft inbox row

- **WHEN** a client posts `POST /api/projects/3/mr-scan?force=1`, triage lists MR IID 12 round 1, and a `draft` row exists for `(project_id=3, mr_iid=12, review_round=1)`
- **THEN** the worker spawns an agent for MR 12 and upserts the draft

#### Scenario: Default manual scan respects inbox gate

- **WHEN** a client posts `POST /api/projects/3/mr-scan` without `force`, triage lists MR IID 12 round 1, and a `draft` row exists for that triple
- **THEN** the worker does not spawn an agent for MR 12

### Requirement: Publishing appends GitLab dedup marker to posted note

When `POST /api/mr-reviews/:id/publish` succeeds, the body posted via `glab mr note` MUST include the marker line `By: AI Agent` (matching triage dedup rules). If the draft body already contains that marker, the handler MUST NOT duplicate it. The stored `published_body` MUST equal the content actually posted to GitLab.

#### Scenario: Publish appends marker when absent

- **WHEN** publish is invoked for a draft whose body is `## Review\n\nLooks good.` with no AI marker
- **THEN** `glab mr note` receives content ending with `By: AI Agent` and `published_body` matches the posted content

#### Scenario: Publish does not duplicate existing marker

- **WHEN** publish is invoked for a draft whose body already ends with `By: AI Agent`
- **THEN** the posted note contains exactly one `By: AI Agent` marker and `published_body` matches the posted content
