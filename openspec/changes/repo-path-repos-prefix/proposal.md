## Why

`projects.yaml` 的 `repo_path` 目前原樣寫入 DB，使用者必須手動寫出 `$DATA_ROOT_DIR/repos/...` 完整路徑，容易與慣例目錄布局不一致而設錯。應讓簡短 slug（如 `test/projectA`）自動對應到 `$DATA_ROOT_DIR/repos/test/projectA`，降低手誤。

## What Changes

- 載入 `projects.yaml` 時，對 `repo_path` 套用解析規則後再寫入 `projects` 表與 git 偵測。
- **Repo slug**（非絕對、且不以 `.` / `..` 開頭）：解析為 `$DATA_ROOT_DIR/repos/<repo_path>`。
- **絕對路徑**：維持不變（向後相容）。
- **明確相對路徑**（以 `.` 或 `..` 開頭）：維持相對於 process cwd 的行為（向後相容既有 `./data/reviewer/repos/...` 寫法）。
- 更新 `projects.yaml` 範例與 README 說明新慣例。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `project-config`：`repo_path` 載入時依 `DATA_ROOT_DIR` 解析 repo slug。

## Impact

- Affected specs：`openspec/specs/project-config/spec.md`（delta）
- Affected code：
  - Modified：`backend/src/projects.rs`、`backend/src/config.rs`（若需暴露解析 helper）、`backend/tests/project_config.rs`、`projects.yaml`、`README.md`
  - New：（無）
  - Removed：（無）
