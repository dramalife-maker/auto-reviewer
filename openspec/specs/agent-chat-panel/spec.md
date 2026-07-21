# agent-chat-panel Specification

## Purpose

TBD - created by archiving change 'extract-agent-chat-panel'. Update Purpose after archive.

## Requirements

### Requirement: Shared Agent Chat panel is presentational

The frontend SHALL render Agent Chat on the person report reader and MR inbox through one shared presentational panel component hosted inside a shared floating launcher chrome (floating button + overlay). The panel MUST own only transcript display and optional composer controls. The launcher MUST own open/close chrome. Page modules MUST retain ownership of chat hydration, agent-turn API calls, optimistic message updates, draft conflict handling, report reload after a successful report-chat turn, and page-specific visibility gates.

#### Scenario: Both surfaces use the shared panel

- **WHEN** an operator opens Agent Chat on the person report reader or on the MR inbox
- **THEN** the visible chat chrome inside the overlay (title, close control, message bubbles, and optional composer) is rendered by the shared panel component
- **AND** the page module still performs the agent-turn request for that surface

#### Scenario: Read-only hides composer

- **WHEN** the shared panel is rendered with read-only mode enabled
- **THEN** the transcript remains visible
- **AND** the composer controls used to submit an agent turn are not available


<!-- @trace
source: agent-chat-floating-button
updated: 2026-07-18
code:
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/pages/MrInboxPage.tsx
  - frontend/src/components/AgentChatLauncher.tsx
  - frontend/src/components/AgentChatPanel.tsx
tests:
  - frontend/src/pages/MrInboxPage.test.tsx
  - frontend/src/pages/ReportsPage.agentChat.test.tsx
  - frontend/src/components/AgentChatPanel.test.tsx
  - frontend/src/components/AgentChatLauncher.test.tsx
-->

---
### Requirement: Page-specific Agent Chat behavior is preserved after extraction

Extracting and floating the shared panel MUST NOT change the existing operator-visible behavior differences between the person report reader and the MR inbox:

- The person report reader SHALL continue to offer Agent Chat whenever a person is selected, allow sending without an `agent_session_id` gate in the UI, roll back the optimistic user bubble and restore the input text when a turn fails, and reload latest reports after a successful turn.
- The MR inbox SHALL continue to offer Agent Chat only for draft reviews or non-draft reviews that already have chat history; non-draft reviews with history MUST be read-only; draft sending MUST remain gated on a non-empty `agent_session_id`; a failed turn MUST NOT roll back the optimistic user bubble; a successful turn MUST continue to feed returned draft body/hash into the existing incoming-draft handling path.

#### Scenario: MR published history stays read-only

- **WHEN** the operator opens a published MR review that has stored chat messages and opens Agent Chat
- **THEN** Agent Chat messages are visible
- **AND** no send control that can submit an agent turn is available

#### Scenario: MR published without history stays hidden

- **WHEN** the operator opens a published MR review with an empty chat transcript
- **THEN** neither the floating expand control nor the Agent Chat overlay is shown

#### Scenario: Report chat failure restores the composer text

- **WHEN** the operator sends a person-report Agent Chat message and the agent-turn request fails
- **THEN** the optimistic user bubble is removed
- **AND** the composer input is restored to the failed message text


<!-- @trace
source: agent-chat-floating-button
updated: 2026-07-18
code:
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/pages/MrInboxPage.tsx
  - frontend/src/components/AgentChatLauncher.tsx
  - frontend/src/components/AgentChatPanel.tsx
tests:
  - frontend/src/pages/MrInboxPage.test.tsx
  - frontend/src/pages/ReportsPage.agentChat.test.tsx
  - frontend/src/components/AgentChatPanel.test.tsx
  - frontend/src/components/AgentChatLauncher.test.tsx
-->

---
### Requirement: Agent Chat opens from a floating button into an overlay

On the person report reader and the MR inbox, when Agent Chat is available, the frontend SHALL present a floating action button that opens Agent Chat in an overlay panel. While the overlay is closed, the page main content MUST occupy the full content width without a dedicated chat side column or collapsed narrow chat rail. The overlay MUST contain the shared Agent Chat panel (title, close control, transcript, and optional composer). Closing the overlay MUST return the operator to the floating button.

#### Scenario: Closed state shows FAB only

- **WHEN** Agent Chat is available and the overlay is closed
- **THEN** a floating control that expands Agent Chat is visible
- **AND** no side-column chat card and no narrow collapsed chat rail are rendered

#### Scenario: FAB opens the overlay panel

- **WHEN** the operator activates the floating expand control
- **THEN** the Agent Chat overlay panel becomes visible
- **AND** the shared Agent Chat panel content is shown inside that overlay

#### Scenario: Close returns to the FAB

- **WHEN** the Agent Chat overlay is open and the operator activates the close control
- **THEN** the overlay is hidden
- **AND** the floating expand control is visible again


<!-- @trace
source: agent-chat-floating-button
updated: 2026-07-18
code:
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/pages/MrInboxPage.tsx
  - frontend/src/components/AgentChatLauncher.tsx
  - frontend/src/components/AgentChatPanel.tsx
tests:
  - frontend/src/pages/MrInboxPage.test.tsx
  - frontend/src/pages/ReportsPage.agentChat.test.tsx
  - frontend/src/components/AgentChatPanel.test.tsx
  - frontend/src/components/AgentChatLauncher.test.tsx
-->

---
### Requirement: Agent Chat overlay defaults to closed

When a surface mounts Agent Chat, the overlay SHALL start closed so only the floating button is shown until the operator opens it.

#### Scenario: Initial mount is closed

- **WHEN** the operator opens a person report or an MR review where Agent Chat is available
- **THEN** the Agent Chat overlay is not shown initially
- **AND** the floating expand control is shown

<!-- @trace
source: agent-chat-floating-button
updated: 2026-07-18
code:
  - frontend/src/pages/ReportsPage.tsx
  - frontend/src/pages/MrInboxPage.tsx
  - frontend/src/components/AgentChatLauncher.tsx
  - frontend/src/components/AgentChatPanel.tsx
tests:
  - frontend/src/pages/MrInboxPage.test.tsx
  - frontend/src/pages/ReportsPage.agentChat.test.tsx
  - frontend/src/components/AgentChatPanel.test.tsx
  - frontend/src/components/AgentChatLauncher.test.tsx
-->

---
### Requirement: Agent Chat floating button and panel are draggable

The frontend SHALL allow the operator to drag both the collapsed floating action button (FAB) and the expanded Agent Chat panel to a new on-screen position. The FAB SHALL be draggable via a pointer-down-and-move interaction on the button itself. The expanded panel SHALL be draggable via a pointer-down-and-move interaction on its header row (the row containing the "Agent Chat" title and close control). Each surface's dragged position MUST be persisted independently (the FAB position MUST NOT overwrite or be overwritten by the panel position) so that reloading the page or navigating away and back restores the last dragged position for each surface. A dragged position MUST remain within the current viewport bounds; if the viewport shrinks (e.g. window resize) below a previously valid position, the position MUST be re-clamped to stay fully visible. A completed drag (pointer movement beyond a small threshold) MUST NOT trigger the FAB's open action on pointer release.

#### Scenario: Dragging the FAB moves and persists its position

- **WHEN** the operator presses down on the floating expand control and moves the pointer before releasing
- **THEN** the control visually follows the pointer movement
- **AND** after release, the new position is retained across a page reload

#### Scenario: Dragging the panel header moves and persists its position

- **WHEN** the Agent Chat overlay is open and the operator presses down on the panel's header row and moves the pointer before releasing
- **THEN** the panel visually follows the pointer movement
- **AND** after release, the new position is retained across a page reload
- **AND** the FAB's stored position is unaffected by this drag

#### Scenario: A completed drag does not trigger the FAB open action

- **WHEN** the operator drags the floating expand control with pointer movement beyond the drag threshold and releases
- **THEN** the Agent Chat overlay does not open as a result of that release

#### Scenario: A plain click without movement still opens the overlay

- **WHEN** the operator presses down and releases the floating expand control without moving the pointer beyond the drag threshold
- **THEN** the Agent Chat overlay opens

#### Scenario: Shrinking the viewport re-clamps an out-of-bounds position

- **WHEN** a previously dragged position for the FAB or the panel falls outside the viewport after the browser window is resized smaller
- **THEN** the position is adjusted to remain fully within the new viewport bounds

##### Example: clamped position on resize

| Stored position (right, bottom) | Element size (w×h) | New viewport (w×h) | Re-clamped position (right, bottom) |
| -------------------------------- | ------------------- | -------------------- | ------------------------------------ |
| (10, 10)                         | 56×56                | 800×600               | (10, 10) — still valid |
| (-20, 10)                        | 56×56                | 800×600               | (0, 10) — right clamped to 0 |
| (10, 700)                        | 56×56                | 800×600               | (10, 544) — bottom clamped to viewportHeight - elementHeight |

<!-- @trace
source: draggable-agent-chat-launcher
updated: 2026-07-20
code:
  - frontend/src/components/AgentChatLauncher.tsx
  - frontend/src/components/ui/Card.tsx
  - frontend/src/lib/useDraggablePosition.ts
  - frontend/src/components/AgentChatPanel.tsx
  - AGENTS.md
tests:
  - frontend/src/components/AgentChatLauncher.test.tsx
-->