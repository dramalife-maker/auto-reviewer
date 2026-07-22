## Why

目前手動觸發週報只能整批跑（`manual_all` 全部專案全部人，或 `manual_project` 單一專案的所有工程師）。當管理者剛修正一位工程師的資料歸戶（補綁 identity、修正 `mr_reviews.person_id`）後，想驗證修正是否生效，只能重跑整個專案的全部人員——這會為其他工程師產生非必要的 LLM 呼叫與 side effect，也擴大重複插入 `pending_items` 的曝險面。需要一個「只重算某一位工程師在某個專案週報」的入口。（issue #4）

## What Changes

- `POST /api/runs` 新增 trigger 類型 `manual_person`，body 帶 `project_name` + `person_id`，回應維持 `{ "run_id": <i64> }`（`201 Created`）。
- `run_projects` 新增可為 NULL 的 `person_id` 欄位（NULL = 整批，沿用現行語意；非 NULL = 只處理該人）。
- 週報 manifest 產生時，若該 run_project 帶 `person_id`，則 `authors` / `open_pending` / `published_pending_snippets` 三塊資料一律過濾為僅該工程師。
- 併發控制沿用既有全系統閘（與 `manual_all` / `manual_project` 一致）：系統有任何 run 在跑即回 409。
- 前端 `humanizeTrigger` 新增 `manual_person` 對應中文標籤（run 歷史顯示用）。
- 前端「人員設定」頁的「參與專案」清單，每個專案加一顆「重跑週報」按鈕：呼叫 `manual_person`（帶該人 `person_id` + 專案 `name`），成功以 toast 顯示已建立、失敗（尤其 409「已有 run 在跑」）以 toast 顯示錯誤。這是管理者修完歸戶後就地驗證的入口。

## Non-Goals

- 不放寬併發模型：不引入「別的專案在跑時仍可插入單人 run」的單專案閘或 per-person 閘。此屬獨立議題。
- 不在 API 端驗證 person 是否「屬於」該專案或該窗口是否有 commit；查無資料時正常產出空 run（no-op），不視為錯誤。
- 前端入口只放在「人員設定」頁的「參與專案」清單（單人重跑最貼近的情境）；不在 Dashboard／Projects／Runs 等其他頁另加人員選擇器或觸發點。
- 不改動 ingest／finalize／cancel／recovery 的既有邏輯。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `reviewer-execution`: 新增 `manual_person` trigger 的 `POST /api/runs` 契約、`run_projects.person_id` 欄位語意、以及週報 manifest 依 `person_id` 過濾 `authors` / `open_pending` / `published_pending_snippets` 的行為。

## Impact

- Affected specs: `reviewer-execution`
- Affected code:
  - New:
    - backend/migrations/015_run_projects_person.sql
  - Modified:
    - backend/src/runs.rs
    - backend/src/server.rs
    - backend/src/worker.rs
    - backend/src/executor.rs
    - frontend/src/lib/format.ts
    - frontend/src/api.ts
    - frontend/src/pages/PeoplePage.tsx
  - Removed: (none)
- Affected tests:
  - backend/tests/runs_execution.rs
  - frontend/src/pages/PeoplePage.rerun.test.tsx
