## ADDED Requirements

### Requirement: MR change materials exclude ignored files from diff content only

When precomputing MR change materials, the worker SHALL apply the global review ignore list by appending exclusion pathspecs to the size-capped `git diff` that produces `change.diff`. The exclusion SHALL NOT be applied to the `git diff --stat` that produces `change_stat.txt`, nor to the `git log --oneline` that produces `change_log.txt`, so that an ignored file remains visible by name and change size to the reviewing agent.

The worker SHALL build each exclusion pathspec by prefixing a stored pattern with git's exclude pathspec magic, and SHALL NOT add glob pathspec magic, so that a wildcard matches across directory levels. When the stored list is empty, the produced materials SHALL be byte-identical to those produced without this feature.

The worker SHALL load the ignore list once per MR scan run before iterating merge requests, rather than once per merge request. A change saved through the settings API therefore takes effect on the next run without restarting the service.

The stub materials path used by the test executor SHALL remain unaffected by the ignore list.

#### Scenario: Ignored file is absent from diff but present in stat

- **WHEN** an MR changes both a source file and a file matching a configured ignore pattern
- **THEN** `change.diff` contains no diff hunk for the matching file, while `change_stat.txt` still lists that file with its insertion and deletion counts

##### Example: lock file alongside source

- **GIVEN** the ignore list is `["*.lock"]` and the MR changes `src/main.rs` and `deps/foo.lock`
- **WHEN** the worker precomputes change materials
- **THEN** `change.diff` contains a hunk for `src/main.rs` and none for `deps/foo.lock`, and `change_stat.txt` lists both files

#### Scenario: Empty list leaves materials unchanged

- **WHEN** the ignore list is empty and the worker precomputes change materials
- **THEN** the produced `change.diff` is identical to the diff produced with no pathspec applied

#### Scenario: List is read once per run

- **WHEN** a single MR scan run reviews multiple eligible merge requests
- **THEN** the ignore list is loaded once for that run and the same list is applied to every merge request in it

### Requirement: Pathspec failure degrades to an unfiltered diff

When the `git diff` invocation carrying exclusion pathspecs fails, the worker SHALL log a warning identifying the failure and SHALL retry the same diff without any pathspec. Only if the retry also fails SHALL the worker report the git error to its caller and let the existing skip handling apply.

A malformed entry in the ignore list SHALL NOT cause merge requests to be skipped while the underlying diff is otherwise obtainable. Failures of the `--stat` and `log` invocations SHALL keep their existing behavior.

#### Scenario: Malformed pattern falls back instead of skipping the MR

- **WHEN** the configured ignore list causes the pathspec-bearing diff to fail
- **THEN** the worker logs a warning, produces `change.diff` from an unfiltered diff, and the merge request is reviewed rather than skipped

#### Scenario: Genuine git failure still propagates

- **WHEN** the diff fails both with and without pathspecs
- **THEN** the worker reports the git error and the existing skip handling for failed change materials applies
