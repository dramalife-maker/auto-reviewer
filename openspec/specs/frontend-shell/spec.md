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