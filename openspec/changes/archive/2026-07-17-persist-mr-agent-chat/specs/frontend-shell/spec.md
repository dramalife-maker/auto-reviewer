## ADDED Requirements

### Requirement: MR inbox hydrates Agent Chat from the API

The MR Inbox view SHALL initialize the Agent Chat transcript from the selected review `chat_messages` field returned by `GET /api/mr-reviews`. Reloading the page while viewing a review that has stored messages MUST show those messages without requiring the operator to re-send them.

#### Scenario: Draft reload shows stored chat

- **WHEN** the operator reloads `#/mr-inbox` and selects a draft whose list payload includes non-empty `chat_messages`
- **THEN** the Agent Chat panel renders those messages in order

### Requirement: Published and ignored Agent Chat is read-only

For `published` or `ignored` reviews that have at least one stored chat message, the MR Inbox SHALL show the Agent Chat transcript in read-only form and MUST NOT offer send controls. For those statuses with an empty transcript, the Agent Chat section MUST be hidden. Draft reviews continue to show the Agent Chat composer when an `agent_session_id` is present.

#### Scenario: Published history without send

- **WHEN** the operator opens a published review that has stored chat messages
- **THEN** the Agent Chat messages are visible
- **AND** no send button or chat input that can submit an agent turn is available

#### Scenario: Published without history hides chat

- **WHEN** the operator opens a published review with empty `chat_messages`
- **THEN** the Agent Chat section is not shown

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

### Requirement: MR inbox save sends base hash

When saving a draft from the MR Inbox, the client MUST send `base_hash` equal to the current baseline `draft_hash`. On HTTP 409 from PATCH, the client MUST surface the conflict using the response `draft_body` / `draft_hash` with the same Preview new version / Load new version / Keep my edits choices and MUST NOT silently overwrite.

#### Scenario: Save conflict surfaces choices

- **WHEN** PATCH returns 409 because `base_hash` is stale
- **THEN** the operator sees Preview new version, Load new version, and Keep my edits
- **AND** the editor is not replaced until Load new version is chosen
