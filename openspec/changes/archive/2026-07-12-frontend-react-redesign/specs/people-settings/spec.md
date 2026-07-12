## MODIFIED Requirements

### Requirement: People settings UI manages persons and identities

The frontend SHALL provide a dedicated people-settings view (separate from the weekly report reader) with:

- a list of people and a control to create a new person
- an editor for the selected person's `display_name`
- identity list with add and remove controls supporting kinds `git_email`, `gitlab_user`, and `glab_user`
- a read-only list of participating project names rendered as a plain bullet list
- an unmatched-authors management section at the top of the people-settings view for binding unmatched authors to an existing person or creating a new person and binding in one action

The people-settings view MUST NOT offer a delete-person action.

The frontend MUST NOT require a global app-header unmatched-authors shortcut panel.

#### Scenario: Create and bind identity from settings view

- **WHEN** a manager creates person "Alice Chen" and binds `git_email` `alice@co.com` from the people-settings view
- **THEN** subsequent `GET /api/people/{id}` shows that identity
- **AND** unmatched authors are manageable from the people-settings unmatched section without using an app-header panel

#### Scenario: Remove identity from settings view

- **WHEN** a manager removes an identity from the selected person in people-settings
- **THEN** that identity no longer appears in `GET /api/people/{id}/identities`

#### Scenario: Bind unmatched author from people settings

- **WHEN** unmatched authors exist and the manager opens People Settings
- **THEN** the unmatched section lists those authors
- **AND** binding one to an existing person decreases the unmatched count without a full page reload
