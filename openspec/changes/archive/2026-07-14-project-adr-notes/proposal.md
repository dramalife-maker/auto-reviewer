## Why

技術選擇類問題若只靠 MR 草稿追問或週報 `待確認`，合併進常駐分支後常會被再次問到；管理者已在 agent chat 回答過的專案事實沒有專案級落點。需要可讀寫的專案 ADR 庫，讓兩軌 agent 先讀再問，避免重複。

## What Changes

- 在 `{DATA_ROOT_DIR}/reports/{project}/.notes/` 建立專案 ADR：`index.md` 僅索引，正文為個別 `adr-YYYYMMDD-slug.md`
- 新 skill 定義顯式寫入規則（管理者說「記成 ADR／寫入決策」才落檔）；MR agent chat（`--resume`）附加該 skill，並可寫入 `.notes/`
- 週報與 MR poll manifest 帶入 `notes_dir`（或等價路徑）；`reviewer-batch` 與 `scan-mrs-headless` workflow **必須**先讀 `index.md`，已知決策不得再當新追問／新 `待確認`
- 釐清與 person `_notes.md`／`pending_items` 的邊界：ADR 是專案事實，不是人物待確認歷史

## Capabilities

### New Capabilities

- `project-adr-notes`: 專案級 ADR 目錄契約、索引／單則格式、寫入觸發（僅顯式）、兩軌必讀與禁止重問

### Modified Capabilities

- `reviewer-execution`: weekly／MR manifest 暴露 `notes_dir`；workflow／agent-turn 消費 ADR 路徑與 chat 寫入 skill
- `person-trends`: 確認 `reports/{project}/.notes/` 不被當成人物目錄或專案名掃描碰撞

## Impact

- Affected specs: `project-adr-notes`（新）、`reviewer-execution`、`person-trends`
- Affected code:
  - New: `skills/project-adr-notes/SKILL.md`（或等價 WORKFLOW／契約檔）、必要時 `skills/project-adr-notes/adr-contract.md`
  - Modified: `backend/src/runs.rs`（`RunManifest`／`MrPollManifest`）、`backend/src/executor.rs`（agent-turn 附加 skill／允許寫入 notes）、`skills/reviewer-batch/WORKFLOW.md`、`skills/reviewer-batch/output-contract.md`、`skills/scan-mrs-headless/WORKFLOW.md`、`skills/scan-mrs-headless/output-contract.md`、對應 `backend/tests/`
  - Removed: （none）

