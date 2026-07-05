## ADDED Requirements

### Requirement: Server initializes data directory and database

The backend SHALL read `REVIEWER_DATA_DIR` from the environment on startup. If the variable is unset, the process MUST exit with a non-zero status and emit an error message naming the variable.

When `REVIEWER_DATA_DIR` is set, the server SHALL create the directory tree roots `repos/` and `reports/` if missing, open or create `reviewer.db` under that directory, run SQL migrations, and enable SQLite foreign keys.

#### Scenario: Successful startup with valid data directory

- **WHEN** the server starts with `REVIEWER_DATA_DIR=/data/reviewer` pointing to a writable path
- **THEN** the server listens for HTTP requests and `reviewer.db` exists under `/data/reviewer`

#### Scenario: Missing environment variable

- **WHEN** the server starts without `REVIEWER_DATA_DIR`
- **THEN** the process exits before binding a port and stderr contains `REVIEWER_DATA_DIR`

### Requirement: Health endpoint reports readiness

The server SHALL expose `GET /health` returning HTTP 200 with JSON body containing `status` equal to `"ok"` and `data_dir` equal to the configured absolute path.

#### Scenario: Health check after startup

- **WHEN** a client sends `GET /health` after successful startup
- **THEN** the response status is 200 and the JSON `status` field is `"ok"`

