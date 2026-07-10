## ADDED Requirements

### Requirement: Unbind identity from person API

The backend SHALL expose `DELETE /api/people/{id}/identities/{identity_id}` that removes the `person_identities` row when it belongs to the given person.

On success the response status MUST be 204.

If the person does not exist, the identity does not exist, or the identity belongs to a different person, the response status MUST be 404.

Deleting the person's last remaining identity MUST be allowed.

#### Scenario: Delete an identity

- **GIVEN** person id 1 has identity id 9
- **WHEN** a client sends `DELETE /api/people/1/identities/9`
- **THEN** the response status is 204
- **AND** `GET /api/people/1/identities` no longer includes identity id 9

#### Scenario: Delete identity for wrong person returns 404

- **GIVEN** identity id 9 belongs to person id 1
- **WHEN** a client sends `DELETE /api/people/2/identities/9`
- **THEN** the response status is 404
- **AND** identity id 9 still exists

#### Scenario: Deleting the last identity is allowed

- **GIVEN** person id 1 has exactly one identity
- **WHEN** that identity is deleted
- **THEN** the response status is 204
- **AND** `GET /api/people/1/identities` returns an empty array

## MODIFIED Requirements

### Requirement: Bind identity to person API

The backend SHALL expose `POST /api/people/:id/identities` accepting JSON `{ "kind": "<string>", "value": "<string>", "label": "<string|null>" }`.

Supported kinds for the people-settings UI MUST include `git_email`, `gitlab_user`, and `glab_user`. The backend MUST continue to normalize `git_email` values by trimming and lowercasing. For `gitlab_user` and `glab_user`, the backend MUST trim whitespace and MUST NOT force lowercase.

On success, the backend MUST insert a `person_identities` row and remove any matching `unmatched_authors` row with the same `(kind, value)`.

If `(kind, value)` is already bound to a different `person_id`, the server MUST respond with HTTP 409.

If `(kind, value)` is already bound to the same `person_id`, the server MUST treat the request as a no-op success without inserting a duplicate row.

#### Scenario: Bind email and clear unmatched queue

- **WHEN** `unmatched_authors` contains `('git_email', 'alice@co.com')` and a client binds that email to person id 1
- **THEN** `person_identities` contains the binding and `unmatched_authors` no longer contains that email

#### Scenario: Reject duplicate identity binding

- **WHEN** `('git_email', 'alice@co.com')` is already bound to person id 1
- **THEN** binding the same email to person id 2 returns HTTP 409

#### Scenario: Bind gitlab_user identity

- **WHEN** a client binds `{ "kind": "gitlab_user", "value": "alice.chen" }` to person id 1
- **THEN** `person_identities` contains that row for person id 1

#### Scenario: Same-person rebind is no-op

- **GIVEN** person id 1 already has `('git_email', 'alice@co.com')`
- **WHEN** a client binds the same kind and value to person id 1 again
- **THEN** the response indicates success
- **AND** only one matching `person_identities` row exists

