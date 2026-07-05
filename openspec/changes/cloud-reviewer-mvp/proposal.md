## Why

團隊有多專案、跨專案工程師，現有 reviewer skill 需逐專案手動執行，產出分散在伺服器 md 檔，缺乏排程、通知與以人為中心的閱讀介面。需要將既有設計（docs/idea/spec.md、schema.md）落地為可運行的雲端 MVP，驗證「每週自動跑、web 上直接看本週報告」的核心價值。

## What Changes

- 新增 Rust 常駐後端：SQLite 設定/狀態、`DATA_ROOT_DIR` 檔案布局、REST API。
- 新增瀏覽器前端 SPA：本週報告閱讀器（選人 → 專案 Tab → 本週內容）。
- 實作軌道 1 批次執行：後端以子行程呼叫 `claude -p` 跑 reviewer-batch，解析 `summary.md` 入庫。
- 實作後端內建排程（`tokio-cron-scheduler`）與「全部執行」手動觸發。
- MVP 以 `projects.yaml`（`name` / `repo_path` / `git_remote_url`）載入專案，暂不建設定 UI。
- 跑完後 web 通知（瀏覽器內通知或簡易 polling 提示，依 design 定案）。
- 趨勢頁、MR 收件匣、人員設定 UI、多使用者認證留待後續 change。

## Capabilities

### New Capabilities

- `backend-foundation`：後端骨架、SQLite migration、`DATA_ROOT_DIR` 目錄初始化、健康檢查 API。
- `project-config`：載入 `projects.yaml` 至 `projects` 表、repo 路徑偵測（git / default_branch）。
- `reviewer-execution`：runs / run_projects 佇列、worker pool、子行程執行 skill、解析 summary.md 寫入 reports / pending_items。
- `scheduling`：`schedule_config` 單列設定、週報 cron、全部執行 API、逾時 skip。
- `report-reader`：本週報告列表/詳情 API 與前端閱讀器（Markdown 渲染、已讀狀態）。

### Modified Capabilities

（無——openspec/specs/ 尚無既有 capability）

## Impact

- Affected specs：新增上述 5 個 capability spec（openspec/changes/cloud-reviewer-mvp/specs/）。
- Affected code：
  - New：`backend/`（Rust API + worker）、`frontend/`（SPA）、`projects.yaml` 範例、`openspec/specs/`（archive 後）
  - Modified：`docs/idea/spec.md`、`docs/idea/schema.md`（僅在實作發現契約偏差時同步）
  - Removed：（無）

