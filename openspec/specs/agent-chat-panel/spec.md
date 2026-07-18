# agent-chat-panel Specification

## Purpose

TBD - created by archiving change 'extract-agent-chat-panel'. Update Purpose after archive.

## Requirements

### Requirement: Shared Agent Chat panel is presentational

The frontend SHALL render Agent Chat on the person report reader and MR inbox through one shared presentational panel component. That panel MUST own only transcript display and optional composer controls. Page modules MUST retain ownership of chat hydration, agent-turn API calls, optimistic message updates, draft conflict handling, report reload after a successful report-chat turn, and page-specific visibility gates.

#### Scenario: Both surfaces use the shared panel

- **WHEN** an operator opens Agent Chat on the person report reader or on the MR inbox
- **THEN** the visible chat chrome (title, collapse control, message bubbles, and optional composer) is rendered by the shared panel component
- **AND** the page module still performs the agent-turn request for that surface

#### Scenario: Read-only hides composer

- **WHEN** the shared panel is rendered with read-only mode enabled
- **THEN** the transcript remains visible
- **AND** the composer controls used to submit an agent turn are not available


<!-- @trace
source: extract-agent-chat-panel
updated: 2026-07-18
code:
  - frontend/src/components/AgentChatPanel.tsx
  - frontend/src/pages/MrInboxPage.tsx
  - frontend/src/pages/ReportsPage.tsx
tests:
  - frontend/src/pages/ReportsPage.agentChat.test.tsx
  - frontend/src/components/AgentChatPanel.test.tsx
-->

---
### Requirement: Page-specific Agent Chat behavior is preserved after extraction

Extracting the shared panel MUST NOT change the existing operator-visible behavior differences between the person report reader and the MR inbox:

- The person report reader SHALL continue to show Agent Chat whenever a person is selected, allow sending without an `agent_session_id` gate in the UI, roll back the optimistic user bubble and restore the input text when a turn fails, and reload latest reports after a successful turn.
- The MR inbox SHALL continue to show Agent Chat only for draft reviews or non-draft reviews that already have chat history; non-draft reviews with history MUST be read-only; draft sending MUST remain gated on a non-empty `agent_session_id`; a failed turn MUST NOT roll back the optimistic user bubble; a successful turn MUST continue to feed returned draft body/hash into the existing incoming-draft handling path.

#### Scenario: MR published history stays read-only

- **WHEN** the operator opens a published MR review that has stored chat messages
- **THEN** Agent Chat messages are visible
- **AND** no send control that can submit an agent turn is available

#### Scenario: MR published without history stays hidden

- **WHEN** the operator opens a published MR review with an empty chat transcript
- **THEN** the Agent Chat section is not shown

#### Scenario: Report chat failure restores the composer text

- **WHEN** the operator sends a person-report Agent Chat message and the agent-turn request fails
- **THEN** the optimistic user bubble is removed
- **AND** the composer input is restored to the failed message text

<!-- @trace
source: extract-agent-chat-panel
updated: 2026-07-18
code:
  - frontend/src/components/AgentChatPanel.tsx
  - frontend/src/pages/MrInboxPage.tsx
  - frontend/src/pages/ReportsPage.tsx
tests:
  - frontend/src/pages/ReportsPage.agentChat.test.tsx
  - frontend/src/components/AgentChatPanel.test.tsx
-->