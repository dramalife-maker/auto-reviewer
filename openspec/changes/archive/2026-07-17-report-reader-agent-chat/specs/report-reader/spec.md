## ADDED Requirements

### Requirement: Report reader hosts person Agent Chat

The report reader frontend SHALL show an Agent Chat panel for the selected person.

- The panel MUST load transcript via `GET /api/people/:id/report-chat` when a person is selected and MUST re-hydrate when the selected person changes.
- The panel MUST allow sending a message via `POST /api/people/:id/report-chat/agent-turn` and append the user message and assistant reply to the visible transcript on success.
- After a successful agent-turn, the panel MUST reload that person's latest reports so file edits made by the agent become visible.
- The panel MUST NOT offer publish or GitLab actions.
- When no person is selected, the Agent Chat panel MUST NOT be interactive for report chat.

#### Scenario: Selecting a person hydrates chat history

- **GIVEN** `GET /api/people/1/report-chat` returns two stored messages
- **WHEN** the operator opens the report reader for person 1
- **THEN** those messages are shown in the Agent Chat panel without re-sending

#### Scenario: Successful turn reloads reports

- **GIVEN** the operator is viewing person 1's reports
- **WHEN** an agent-turn succeeds
- **THEN** the client requests latest reports for person 1 again
