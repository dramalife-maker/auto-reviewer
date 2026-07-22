## Why

目前 run 一旦開始就沒有煞車。專案卡在無回應的 agent subprocess、使用者發現觸發錯了、或某個專案要跑很久而使用者想改跑別的，唯一手段是等 per-project timeout 到期或整個重啟後端行程。重啟會讓所有進行中的專案都被標成 `interrupted by shutdown`，是把整批工作連坐犧牲掉。

行程層級的 shutdown token 已經貫穿 worker 與 executor，中止所需的取消傳播機制已經存在，缺的只是一個使用者可觸發、且範圍限縮在單一 run 的入口。

## What Changes

- 新增 `POST /api/runs/{id}/cancel`，中止指定 run 底下所有專案
- `RunWorker` 維護 run_id 到 `CancellationToken` 的登錄表，每個 token 由行程 shutdown token 派生，因此 shutdown 仍能穿透
- 執行中的專案：取消 token 觸發，executor 殺掉 subprocess，該 run_project 落入新的 `cancelled` 終態
- 尚未開始的專案：立即標為 `cancelled`，不再啟動
- `runs.status` 新增 `cancelled` 終態；認領查詢過濾 `r.status = 'running'`，所以標記後自動停止認領新專案
- `finalize_run_if_complete` 不得把已中止的 run 覆寫回 `success`/`partial`/`failed`
- 中止時已寫到磁碟的產出保留並照常 ingest，與現有 `skipped_timeout` 路徑的行為一致
- 前端 run 清單與詳情呈現 `cancelled` 狀態，並在進行中的 run 上提供中止操作
- 行程重啟後的殘留列復原邏輯需認得 `cancelled`，不得將其誤判為需要復原的中斷列

## Non-Goals

- **不做單一專案中止**：只提供整個 run 的中止。per-run_project 粒度需要更細的 token 登錄表與另一組前端操作，而目前的實際需求是「這批我不要了」
- **不做暫停與續跑**：中止是終態，沒有 resume
- **不刪除已產生的產出**：使用者可能正是想保留已跑完的部分才喊停
- **不改 per-project timeout 行為**：`skipped_timeout` 路徑維持原樣
- **不做中止權限控管**：目前後端無認證機制，授權設計是另一個獨立主題

## Capabilities

### New Capabilities

- `run-cancellation`: 使用者主動中止進行中的 run，涵蓋 API 入口、取消傳播、`cancelled` 終態語意，以及與行程 shutdown 的區辨

### Modified Capabilities

- `reviewer-execution`: run 生命週期新增 `cancelled` 終態；run 完結判定不得覆寫已中止的 run
- `run-history`: run 清單與詳情需呈現 `cancelled` 狀態，並在進行中的 run 上提供中止入口
- `graceful-shutdown`: per-run 取消 token 由行程 shutdown token 派生，shutdown 仍須穿透；被中止與被 shutdown 中斷的專案須落入不同終態

## Impact

- Affected specs: `run-cancellation`（新增）、`reviewer-execution`、`run-history`、`graceful-shutdown`
- Affected code:
  - New:
    - openspec/changes/cancel-run/specs/run-cancellation/spec.md
    - backend/tests/run_cancellation.rs
  - Modified:
    - backend/src/worker.rs
    - backend/src/runs.rs
    - backend/src/server.rs
    - backend/src/state.rs
    - backend/src/error.rs
    - frontend/src/pages/RunsPage.tsx
    - frontend/src/pages/RunsPage.test.tsx
    - frontend/src/hooks/useRunPolling.ts
  - Removed: (none)
- 無資料庫 schema 變更：`runs.status` 與 `run_projects.state` 皆為無 CHECK 限制的 TEXT 欄位，新增終態值不需 migration
