# frontend-shell Specification

## Purpose

TBD - created by archiving change 'frontend-react-redesign'. Update Purpose after archive.

## Requirements

### Requirement: Frontend uses React shell with Tailwind design system

The frontend SHALL be implemented as a React + TypeScript single-page application styled with Tailwind CSS utilities derived from the Reviewer redesign design tokens (cool slate neutrals, indigo primary accent `#4f46e5`, MR-track violet `#7c3aed` reserved for MR inbox only, flat borders with no drop shadows).

The application MUST NOT use the legacy vanilla `ReviewerApp` innerHTML rendering path as its primary UI.

#### Scenario: App boots into React shell

- **WHEN** an operator opens the frontend entry page with a healthy backend
- **THEN** the React application mounts and renders the persistent sidebar shell and a main content area
- **AND** the brand block shows a connected status line sourced from `GET /health`


<!-- @trace
source: frontend-react-redesign
updated: 2026-07-12
code:
  - docs/design_handoff_reviewer_redesign/support.js
  - frontend/vite.config.ts
  - frontend/tsconfig.json
  - frontend/src/components/ui/Tabs.tsx
  - frontend/src/lib/catchup.ts
  - frontend/src/components/ui/Card.tsx
  - frontend/src/components/layout/Banner.tsx
  - frontend/src/pages/MrInboxPage.tsx
  - frontend/src/lib/format.ts
  - frontend/src/App.tsx
  - frontend/src/components/layout/Sidebar.tsx
  - frontend/src/components/ui/Input.tsx
  - frontend/src/app.ts
  - frontend/src/components/ui/index.ts
  - frontend/src/lib/icons.ts
  - frontend/src/main.tsx
  - docs/design_handoff_reviewer_redesign/README.md
  - frontend/package.json
  - docs/design_handoff_reviewer_redesign/Reviewer Redesign.dc.html
  - frontend/index.html
  - frontend/src/components/ui/Button.tsx
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/components/ui/NavItem.tsx
  - frontend/src/components/ui/ListRow.tsx
  - frontend/src/components/ui/Badge.tsx
  - frontend/src/style.css
  - frontend/src/components/ui/StatusPill.tsx
  - frontend/src/index.css
  - frontend/src/pages/RunsPage.tsx
  - frontend/src/hooks/useRunPolling.ts
  - frontend/src/components/ui/Avatar.tsx
  - frontend/src/components/ui/StatCard.tsx
  - frontend/src/lib/tokens.ts
  - frontend/src/context/BannerContext.tsx
  - frontend/src/pages/PeoplePage.tsx
  - frontend/src/pages/ProjectsPage.tsx
  - frontend/src/main.ts
  - frontend/src/pages/DashboardPage.tsx
  - frontend/src/hooks/useApi.ts
tests:
  - frontend/src/hooks/useApi.test.ts
  - frontend/src/pages/PeoplePage.unmatched.test.tsx
  - frontend/src/test/setup.ts
  - frontend/src/components/ui/atoms.test.tsx
  - frontend/src/pages/MrInboxPage.test.tsx
  - frontend/src/components/layout/Banner.test.tsx
  - frontend/src/pages/DashboardPage.catchup.test.tsx
  - frontend/src/lib/catchup.test.ts
  - frontend/src/lib/format.test.ts
  - frontend/src/lib/icons.test.ts
  - frontend/src/theme.test.ts
  - frontend/src/App.routes.test.tsx
-->

---
### Requirement: Hash routes map to the six primary views

The frontend SHALL use hash-based client routing for these views:

| Hash path | View |
| --------- | ---- |
| `#/` or `#/dashboard` | Dashboard |
| `#/mr-inbox` | MR Inbox |
| `#/reports` and `#/reports/:personId` | Reports Reader |
| `#/runs` and `#/runs/:runId` | Runs History |
| `#/projects` | Project Settings |
| `#/people` | People Settings |

MR inbox status filter MUST be represented as the query parameter `status` with values `draft`, `published`, or `ignored`.

Reloading the browser on a hash path MUST restore that same view (and person/run id when present in the path).

#### Scenario: Reload preserves reports person route

- **WHEN** the operator is on `#/reports/3` and reloads the page
- **THEN** the Reports Reader view for person id 3 is shown again without requiring a full server-side navigation

#### Scenario: MR filter encoded in query

- **WHEN** the operator selects the published filter in MR Inbox
- **THEN** the location hash includes `status=published`
- **AND** the list shows published reviews from `GET /api/mr-reviews?status=published`


<!-- @trace
source: frontend-react-redesign
updated: 2026-07-12
code:
  - docs/design_handoff_reviewer_redesign/support.js
  - frontend/vite.config.ts
  - frontend/tsconfig.json
  - frontend/src/components/ui/Tabs.tsx
  - frontend/src/lib/catchup.ts
  - frontend/src/components/ui/Card.tsx
  - frontend/src/components/layout/Banner.tsx
  - frontend/src/pages/MrInboxPage.tsx
  - frontend/src/lib/format.ts
  - frontend/src/App.tsx
  - frontend/src/components/layout/Sidebar.tsx
  - frontend/src/components/ui/Input.tsx
  - frontend/src/app.ts
  - frontend/src/components/ui/index.ts
  - frontend/src/lib/icons.ts
  - frontend/src/main.tsx
  - docs/design_handoff_reviewer_redesign/README.md
  - frontend/package.json
  - docs/design_handoff_reviewer_redesign/Reviewer Redesign.dc.html
  - frontend/index.html
  - frontend/src/components/ui/Button.tsx
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/components/ui/NavItem.tsx
  - frontend/src/components/ui/ListRow.tsx
  - frontend/src/components/ui/Badge.tsx
  - frontend/src/style.css
  - frontend/src/components/ui/StatusPill.tsx
  - frontend/src/index.css
  - frontend/src/pages/RunsPage.tsx
  - frontend/src/hooks/useRunPolling.ts
  - frontend/src/components/ui/Avatar.tsx
  - frontend/src/components/ui/StatCard.tsx
  - frontend/src/lib/tokens.ts
  - frontend/src/context/BannerContext.tsx
  - frontend/src/pages/PeoplePage.tsx
  - frontend/src/pages/ProjectsPage.tsx
  - frontend/src/main.ts
  - frontend/src/pages/DashboardPage.tsx
  - frontend/src/hooks/useApi.ts
tests:
  - frontend/src/hooks/useApi.test.ts
  - frontend/src/pages/PeoplePage.unmatched.test.tsx
  - frontend/src/test/setup.ts
  - frontend/src/components/ui/atoms.test.tsx
  - frontend/src/pages/MrInboxPage.test.tsx
  - frontend/src/components/layout/Banner.test.tsx
  - frontend/src/pages/DashboardPage.catchup.test.tsx
  - frontend/src/lib/catchup.test.ts
  - frontend/src/lib/format.test.ts
  - frontend/src/lib/icons.test.ts
  - frontend/src/theme.test.ts
  - frontend/src/App.routes.test.tsx
-->

---
### Requirement: Sidebar navigation groups workbench and settings

The frontend SHALL render a fixed-width sidebar (232px) with:

- brand title "Reviewer" and connection status
- group label "工作台" containing Dashboard, MR Inbox (with violet draft-count badge when count > 0), Reports Reader (expandable person sub-list when active), and Runs History
- group label "設定" containing Project Settings and People Settings (People Settings MUST show a badge when unmatched author count > 0)
- footer version text

Clicking a Reports Reader person row MUST navigate to `#/reports/:personId` and select that person for the reader.

#### Scenario: Reports person sub-list navigates

- **GIVEN** people exist with open pending counts
- **WHEN** the operator activates Reports Reader and clicks a person row
- **THEN** the app navigates to that person's reports route
- **AND** pending-count badges appear only for people with open pending items greater than zero


<!-- @trace
source: frontend-react-redesign
updated: 2026-07-12
code:
  - docs/design_handoff_reviewer_redesign/support.js
  - frontend/vite.config.ts
  - frontend/tsconfig.json
  - frontend/src/components/ui/Tabs.tsx
  - frontend/src/lib/catchup.ts
  - frontend/src/components/ui/Card.tsx
  - frontend/src/components/layout/Banner.tsx
  - frontend/src/pages/MrInboxPage.tsx
  - frontend/src/lib/format.ts
  - frontend/src/App.tsx
  - frontend/src/components/layout/Sidebar.tsx
  - frontend/src/components/ui/Input.tsx
  - frontend/src/app.ts
  - frontend/src/components/ui/index.ts
  - frontend/src/lib/icons.ts
  - frontend/src/main.tsx
  - docs/design_handoff_reviewer_redesign/README.md
  - frontend/package.json
  - docs/design_handoff_reviewer_redesign/Reviewer Redesign.dc.html
  - frontend/index.html
  - frontend/src/components/ui/Button.tsx
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/components/ui/NavItem.tsx
  - frontend/src/components/ui/ListRow.tsx
  - frontend/src/components/ui/Badge.tsx
  - frontend/src/style.css
  - frontend/src/components/ui/StatusPill.tsx
  - frontend/src/index.css
  - frontend/src/pages/RunsPage.tsx
  - frontend/src/hooks/useRunPolling.ts
  - frontend/src/components/ui/Avatar.tsx
  - frontend/src/components/ui/StatCard.tsx
  - frontend/src/lib/tokens.ts
  - frontend/src/context/BannerContext.tsx
  - frontend/src/pages/PeoplePage.tsx
  - frontend/src/pages/ProjectsPage.tsx
  - frontend/src/main.ts
  - frontend/src/pages/DashboardPage.tsx
  - frontend/src/hooks/useApi.ts
tests:
  - frontend/src/hooks/useApi.test.ts
  - frontend/src/pages/PeoplePage.unmatched.test.tsx
  - frontend/src/test/setup.ts
  - frontend/src/components/ui/atoms.test.tsx
  - frontend/src/pages/MrInboxPage.test.tsx
  - frontend/src/components/layout/Banner.test.tsx
  - frontend/src/pages/DashboardPage.catchup.test.tsx
  - frontend/src/lib/catchup.test.ts
  - frontend/src/lib/format.test.ts
  - frontend/src/lib/icons.test.ts
  - frontend/src/theme.test.ts
  - frontend/src/App.routes.test.tsx
-->

---
### Requirement: Feature parity for non-prototype actions

The redesigned UI SHALL preserve these existing operator actions wired to the current API layer:

- schedule catch-up confirm and session-only dismiss on the dashboard when `missed_weekly_run` is present
- force MR scan from project settings in addition to normal MR scan
- unmatched author bind-to-existing and bind-via-new-person flows (hosted on People Settings per person-identity / people-settings deltas)

#### Scenario: Force MR scan remains available

- **WHEN** the operator opens a GitLab-source project in Project Settings
- **THEN** both normal and force MR scan actions are available
- **AND** invoking force scan calls the existing MR scan API with force enabled

<!-- @trace
source: frontend-react-redesign
updated: 2026-07-12
code:
  - docs/design_handoff_reviewer_redesign/support.js
  - frontend/vite.config.ts
  - frontend/tsconfig.json
  - frontend/src/components/ui/Tabs.tsx
  - frontend/src/lib/catchup.ts
  - frontend/src/components/ui/Card.tsx
  - frontend/src/components/layout/Banner.tsx
  - frontend/src/pages/MrInboxPage.tsx
  - frontend/src/lib/format.ts
  - frontend/src/App.tsx
  - frontend/src/components/layout/Sidebar.tsx
  - frontend/src/components/ui/Input.tsx
  - frontend/src/app.ts
  - frontend/src/components/ui/index.ts
  - frontend/src/lib/icons.ts
  - frontend/src/main.tsx
  - docs/design_handoff_reviewer_redesign/README.md
  - frontend/package.json
  - docs/design_handoff_reviewer_redesign/Reviewer Redesign.dc.html
  - frontend/index.html
  - frontend/src/components/ui/Button.tsx
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/components/ui/NavItem.tsx
  - frontend/src/components/ui/ListRow.tsx
  - frontend/src/components/ui/Badge.tsx
  - frontend/src/style.css
  - frontend/src/components/ui/StatusPill.tsx
  - frontend/src/index.css
  - frontend/src/pages/RunsPage.tsx
  - frontend/src/hooks/useRunPolling.ts
  - frontend/src/components/ui/Avatar.tsx
  - frontend/src/components/ui/StatCard.tsx
  - frontend/src/lib/tokens.ts
  - frontend/src/context/BannerContext.tsx
  - frontend/src/pages/PeoplePage.tsx
  - frontend/src/pages/ProjectsPage.tsx
  - frontend/src/main.ts
  - frontend/src/pages/DashboardPage.tsx
  - frontend/src/hooks/useApi.ts
tests:
  - frontend/src/hooks/useApi.test.ts
  - frontend/src/pages/PeoplePage.unmatched.test.tsx
  - frontend/src/test/setup.ts
  - frontend/src/components/ui/atoms.test.tsx
  - frontend/src/pages/MrInboxPage.test.tsx
  - frontend/src/components/layout/Banner.test.tsx
  - frontend/src/pages/DashboardPage.catchup.test.tsx
  - frontend/src/lib/catchup.test.ts
  - frontend/src/lib/format.test.ts
  - frontend/src/lib/icons.test.ts
  - frontend/src/theme.test.ts
  - frontend/src/App.routes.test.tsx
-->

---
### Requirement: MR inbox hydrates Agent Chat from the API

The MR Inbox view SHALL initialize the Agent Chat transcript from the selected review `chat_messages` field returned by `GET /api/mr-reviews`. Reloading the page while viewing a review that has stored messages MUST show those messages without requiring the operator to re-send them.

#### Scenario: Draft reload shows stored chat

- **WHEN** the operator reloads `#/mr-inbox` and selects a draft whose list payload includes non-empty `chat_messages`
- **THEN** the Agent Chat panel renders those messages in order


<!-- @trace
source: persist-mr-agent-chat
updated: 2026-07-17
code:
  - frontend/src/api.ts
  - backend/migrations/014_person_report_chat.sql
  - backend/src/report_chat.rs
  - backend/src/mr_change_materials.rs
  - backend/src/lib.rs
  - frontend/src/types.ts
  - backend/src/summary.rs
  - backend/src/executor.rs
  - backend/src/server.rs
  - backend/src/reports.rs
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/worker.rs
tests:
  - backend/tests/fixtures/report_chat_fail.cmd
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/report_reader.rs
-->

---
### Requirement: Published and ignored Agent Chat is read-only

For `published` or `ignored` reviews that have at least one stored chat message, the MR Inbox SHALL show the Agent Chat transcript in read-only form and MUST NOT offer send controls. For those statuses with an empty transcript, the Agent Chat section MUST be hidden. Draft reviews continue to show the Agent Chat composer when an `agent_session_id` is present.

#### Scenario: Published history without send

- **WHEN** the operator opens a published review that has stored chat messages
- **THEN** the Agent Chat messages are visible
- **AND** no send button or chat input that can submit an agent turn is available

#### Scenario: Published without history hides chat

- **WHEN** the operator opens a published review with empty `chat_messages`
- **THEN** the Agent Chat section is not shown


<!-- @trace
source: persist-mr-agent-chat
updated: 2026-07-17
code:
  - frontend/src/api.ts
  - backend/migrations/014_person_report_chat.sql
  - backend/src/report_chat.rs
  - backend/src/mr_change_materials.rs
  - backend/src/lib.rs
  - frontend/src/types.ts
  - backend/src/summary.rs
  - backend/src/executor.rs
  - backend/src/server.rs
  - backend/src/reports.rs
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/worker.rs
tests:
  - backend/tests/fixtures/report_chat_fail.cmd
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/report_reader.rs
-->

---
### Requirement: Draft editor tracks server baseline and new versions

The MR Inbox draft editor SHALL keep a server baseline (`draft_body` / `draft_hash` from the last successful load, agent-turn, or save). After a successful agent-turn whose returned `draft_body` differs from the baseline: if the editor is not dirty, the editor MUST adopt the returned body, update the baseline, and show a dismissible "draft has a new version" marker on the draft section; if the editor is dirty, the UI MUST show a conflict prompt with "Preview new version", "Load new version" (discard local edits), and "Keep my edits" (retain editor text; a later save SHALL be allowed to overwrite disk only after an explicit Keep choice and a subsequent save). The UI MUST NOT auto-merge conflicting texts. While the editor is dirty, an external update to the selected review's `draft_body` in client state MUST NOT reset the editor contents.

#### Scenario: Clean editor adopts agent draft

- **WHEN** agent-turn returns a `draft_body` different from baseline and the editor is not dirty
- **THEN** the editor shows the returned body
- **AND** a new-version marker is visible on the draft section

#### Scenario: Dirty editor conflict choices

- **WHEN** agent-turn returns a `draft_body` different from baseline and the editor has unsaved local edits
- **THEN** a conflict prompt offers Preview new version, Load new version, and Keep my edits
- **AND** the editor text is unchanged until the operator chooses Load new version

#### Scenario: Preview new version is read-only

- **WHEN** the operator chooses Preview new version during a dirty conflict
- **THEN** the UI shows the server `draft_body` in a read-only preview (Markdown preview is allowed)
- **AND** the editor contents remain the operator's unsaved text


<!-- @trace
source: persist-mr-agent-chat
updated: 2026-07-17
code:
  - frontend/src/api.ts
  - backend/migrations/014_person_report_chat.sql
  - backend/src/report_chat.rs
  - backend/src/mr_change_materials.rs
  - backend/src/lib.rs
  - frontend/src/types.ts
  - backend/src/summary.rs
  - backend/src/executor.rs
  - backend/src/server.rs
  - backend/src/reports.rs
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/worker.rs
tests:
  - backend/tests/fixtures/report_chat_fail.cmd
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/report_reader.rs
-->

---
### Requirement: MR inbox save sends base hash

When saving a draft from the MR Inbox, the client MUST send `base_hash` equal to the current baseline `draft_hash`. On HTTP 409 from PATCH, the client MUST surface the conflict using the response `draft_body` / `draft_hash` with the same Preview new version / Load new version / Keep my edits choices and MUST NOT silently overwrite.

#### Scenario: Save conflict surfaces choices

- **WHEN** PATCH returns 409 because `base_hash` is stale
- **THEN** the operator sees Preview new version, Load new version, and Keep my edits
- **AND** the editor is not replaced until Load new version is chosen

<!-- @trace
source: persist-mr-agent-chat
updated: 2026-07-17
code:
  - frontend/src/api.ts
  - backend/migrations/014_person_report_chat.sql
  - backend/src/report_chat.rs
  - backend/src/mr_change_materials.rs
  - backend/src/lib.rs
  - frontend/src/types.ts
  - backend/src/summary.rs
  - backend/src/executor.rs
  - backend/src/server.rs
  - backend/src/reports.rs
  - frontend/src/pages/ReportsPage.tsx
  - backend/src/worker.rs
tests:
  - backend/tests/fixtures/report_chat_fail.cmd
  - backend/tests/fixtures/report_chat_ok.cmd
  - backend/tests/report_chat.rs
  - backend/tests/fixtures/report_chat_ok.sh
  - backend/tests/fixtures/report_chat_ok.py
  - backend/tests/fixtures/report_chat_fail.sh
  - backend/tests/report_reader.rs
-->