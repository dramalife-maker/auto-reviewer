## Why

管理者在報告閱讀器看週報時，看不到仍留在 `_pending/`、尚未被週報折入的 MR 觀察片段（draft／ignored／已 published 但尚未消費）。這些片段是 1on1 與成長討論的重要訊號，目前只能到 MR 收件匣或檔案系統找，閱讀器與觀察脈絡脫節。

## What Changes

- 擴充 `GET /api/people/:id/reports/latest`：每個專案卡片新增 `pending_observations` 陣列，列出該人在該專案 `_pending/` 下仍存在的觀察片段
- 每個片段回傳：檔名推得的 `mr_iid`／`review_round`、正文 markdown、對應 `mr_reviews.status`（無對應列時為 `unknown`）
- 報告閱讀器總覽與專案 tab 新增「待折入觀察」區塊，顯示全文與 status 標籤；與既有「待確認」（SQLite `pending_items`）並列、語意分開

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `report-reader`: 最新週報 API 與閱讀器 UI 必須暴露尚未被週報消費的 MR 觀察片段

## Impact

- Affected specs: `report-reader`
- Affected code:
  - Modified: `backend/src/reports.rs`, `backend/tests/report_reader.rs`, `frontend/src/types.ts`, `frontend/src/pages/ReportsPage.tsx`, `openspec/specs/report-reader/spec.md`
  - New: （無獨立新模組；片段載入邏輯放在 `backend/src/reports.rs` 或薄包裝於既有 `mr_reviews` 路徑輔助）
