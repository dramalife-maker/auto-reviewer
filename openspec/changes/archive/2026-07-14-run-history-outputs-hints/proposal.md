## Why

執行紀錄詳情的「專案結果」目前只顯示狀態、錯誤與 MR skip 摘要；管理者在 run 結束後無法立刻知道這次是否產出了 MR 草稿或週報、該去哪個畫面看。管理者需要在同一處得到產出提示與導向連結。

## What Changes

- `GET /api/runs/{id}` 在 run 非 `running` 時，為每個專案附上可選 `outputs`：MR 草稿數量、本次寫入的週報人員清單（`person_id` + `display_name`）
- 執行紀錄詳情「專案結果」卡顯示產出摘要，並提供可點擊連結至 MR 收件匣與報告閱讀器
- 無論專案 state 為成功或失敗，只要有產出即顯示提示
- 擴充 `run-history` 規格與對應測試

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `run-history`: 詳情 API／UI 須揭露專案產出（MR 草稿、週報）並導向既有閱讀頁

## Impact

- Affected specs: `run-history`
- Affected code:
  - Modified: `backend/src/runs.rs`, `backend/src/server.rs`, `frontend/src/types.ts`, `frontend/src/pages/RunsPage.tsx`, `openspec/specs/run-history/spec.md`
  - New: 後端／前端測試檔（細節見 design／tasks）
  - Removed: (none)
