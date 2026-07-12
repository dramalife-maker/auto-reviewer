## MODIFIED Requirements

### Requirement: Frontend exposes unmatched author management

The frontend SHALL display the count of unmatched authors on the People Settings navigation item when the count is greater than zero, and SHALL provide an unmatched-authors section inside the People Settings view to bind each unmatched author to an existing person or create a new person and bind in one action.

The frontend MUST NOT require a separate global header unmatched-authors panel as the primary entry point.

#### Scenario: Bind unmatched author from UI

- **WHEN** the user selects an unmatched author in People Settings and chooses an existing person to bind
- **THEN** the unmatched count decreases and the binding succeeds without a full page reload

##### Example: bind from people-settings unmatched section

- **GIVEN** unmatched author `alice@gmail.com` on project `game-backend` and existing person id 1 "Alice Chen"
- **WHEN** the user binds that unmatched email to person id 1 from People Settings
- **THEN** `GET /api/unmatched-authors` no longer lists `alice@gmail.com` and person id 1 has `identity_count` increased by 1

#### Scenario: Nav badge reflects unmatched count

- **WHEN** at least one unmatched author exists
- **THEN** the People Settings sidebar item shows a count badge
- **AND** when the unmatched list becomes empty after binding, the badge is hidden
