## Why

目前 `axum::serve` 無 graceful shutdown：Ctrl+C／SIGTERM 直接砍進程，HTTP、worker、cron、agent 子行程無協調收尾；Windows 上常留下 `cursor-agent`／node 孤兒，且 DB 的 `run_projects` 可能永久停在 `running`，UI 假死「執行中」。

## What Changes

- 進程對 Ctrl+C 與 Unix SIGTERM 啟動協調關機（硬上限 15 秒）
- HTTP 停收新連線並結束 in-flight request
- 停止 cron 排程與 worker 接新 job
- 以共用 `CancellationToken` 取消 in-flight executor（含 weekly／MR scan／HTTP `agent-turn`），並 `kill_process_tree`
- 被中斷的 `running` `run_projects` 標 `failed`（error：`interrupted by shutdown`），並 finalize 對應 runs；`queued` 維持不動供重啟後續跑
- 啟動時補償：將殘留 `running` 列標 `failed`（error：`interrupted by previous shutdown`）並 finalize

## Capabilities

### New Capabilities

- `graceful-shutdown`: 進程信號、關機時序、HTTP／worker／scheduler 協調、啟動殘留補償、15s 硬上限

### Modified Capabilities

- `reviewer-execution`: executor 與 `agent-turn` 必須接受並遵守 `CancellationToken`；取消時殺子行程樹並回傳失敗（非 timeout skip）

## Impact

- Affected specs: `graceful-shutdown`（新）、`reviewer-execution`（delta）
- Affected code:
  - Modified: `backend/src/lib.rs`, `backend/src/worker.rs`, `backend/src/executor.rs`, `backend/src/schedule.rs`, `backend/src/state.rs`, `backend/src/runs.rs`, `backend/src/mr_reviews.rs`, `backend/Cargo.toml`
  - New: `backend/tests/` 下關機／啟動補償相關測試（具體檔名於 design／tasks 定）
