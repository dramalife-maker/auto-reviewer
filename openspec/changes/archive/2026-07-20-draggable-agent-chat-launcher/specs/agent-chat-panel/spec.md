## ADDED Requirements

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

