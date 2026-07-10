## Why

管理者目前只能看到「上次執行」與進行中的 run polling，無法查歷史批次結果，也看不到 MR triage／inbox-gate 為何跳過某些 MR。除錯「為什麼這週沒報告／為什麼 MR 沒掃」只能翻伺服器 log 與 `eligible_mrs.json`。雙軌已落地，需要可觀測的執行紀錄與 skip 摘要。

## What Changes

- 新增 `GET /api/runs` 分頁列表（trigger／status／時間／專案計數）。
- 擴充 `GET /api/runs/{id}`：補齊 `duration_sec`、各 `run_projects` 的耗時與錯誤；MR 類 trigger 附上 skip 摘要（讀既有 `eligible_mrs.json` 的 `skipped[]`，含 triage 與 inbox-gate 原因）。
- 控制台顯示最近執行並可進入完整「執行紀錄」視圖（列表 → 明細）；側欄提供入口。
- **不**新增 skip 專用 DB 表（檔案為準）；**不**做 log 串流、stdout 全文、刪除 run、數量圖表。

## Capabilities

### New Capabilities

- `run-history`: 執行紀錄列表／明細 API 與前端視圖，含 MR skip 摘要呈現。

### Modified Capabilities

- `reviewer-execution`: dashboard 回傳 `recent_runs`（最近 5 筆）供控制台入口。

## Impact

- Affected specs: run-history (new), reviewer-execution (modified)
- Affected code:
  - New: (helpers may live in `backend/src/runs.rs`)
  - Modified: `backend/src/runs.rs`, `backend/src/server.rs`, `backend/src/mr_reviews.rs` (read skipped summary), `backend/tests/runs_execution.rs`, `frontend/src/app.ts`, `frontend/src/api.ts`, `frontend/src/types.ts`, `frontend/src/style.css`, `docs/idea/schema.md`, `README.md`
  - Removed: (none)


