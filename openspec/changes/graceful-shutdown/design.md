## Context

後端是單一 tokio 進程：`run()` 綁 HTTP、`RunWorker` 背景 drain queue、`tokio-cron-scheduler` 觸發週報／MR poll，`executor` 以子行程跑 agent（Windows 常經 cmd／powershell 再進 node）。目前無信號處理；timeout 路徑已有 `kill_process_tree`（Windows `taskkill /F /T`），但進程退出不會走該路徑。

## Goals / Non-Goals

**Goals:**

- Ctrl+C 與 Unix SIGTERM 觸發同一關機序列
- 停 HTTP 新連線、停 cron、停 worker 接新 job
- 取消 in-flight executor（含 `agent-turn`），殺子行程樹
- 中斷中的 `running` 列標 `failed`（`interrupted by shutdown`）並 finalize run；`queued` 不動
- 啟動補償殘留 `running` → `failed`（`interrupted by previous shutdown`）
- 關機硬上限 15 秒

**Non-Goals:**

- 不新增 `cancelled`／`interrupted` 狀態（不 migration）
- 不新增 `REVIEWER_SHUTDOWN_TIMEOUT_SEC` 等可調 env
- 不做第二下 Ctrl+C 立即 `exit`
- 不做全域 PID registry
- 不改前端 UI 文案（沿用既有 failed + error 顯示）
- 不處理多實例／分散式 lock

## Decisions

### Decision: 全層關機（HTTP + worker／scheduler + kill 子行程）

關機必須同時停收 HTTP、停排程、取消 worker，並殺 agent 子行程樹。只關 HTTP 會留下 Windows 孤兒與 `running` 假死。

Alternatives: 僅 HTTP graceful；HTTP + 等子行程跑完不殺 — 皆拒（關機不可預期、易卡死）。

### Decision: 中斷終態為 failed

被關機取消的 `run_projects` MUST 設 `state='failed'`、`error` 含精確字串 `interrupted by shutdown`；對應 `runs` 經既有 `finalize_run_if_complete` 收斂。`queued` 列 MUST 保持 `queued`。

Alternatives: 新狀態 `cancelled`；沿用 `skipped_timeout` — 皆拒（schema／語意成本或與 timeout 混淆）。

### Decision: 固定 15 秒關機硬上限

自收到信號起，cleanup 逾 15 秒則強制結束進程。常數即可，不加 env。

### Decision: 啟動補償殘留 running

`init_app`（或同等啟動路徑）在 worker 開始前，將所有 `run_projects.state='running'` 更新為 `failed`（error：`interrupted by previous shutdown`），並對受影響 `run_id` 呼叫 finalize。涵蓋 `kill -9`／強制關機後重啟。

### Decision: CancellationToken 傳遞取消

根 token 放在 `AppState`；信號觸發後 `cancel()`。`execute_weekly_batch`／`execute_mr_review`／`execute_agent_turn` 對 `child.wait()` 與 `cancel.cancelled()` 做 `select!`；cancel 贏則 `kill_process_tree`，回傳失敗（非 `SkippedTimeout`）。Worker loop 與 drain 在 token cancelled 後不再 dequeue。Cron 在 shutdown 時 `shutdown`／停止 scheduler。

Dependencies: 新增 `tokio-util`（`CancellationToken`）若尚未存在。

### Decision: 信號為 Ctrl+C + Unix SIGTERM

`tokio::signal::ctrl_c`；Unix 另聽 `SIGTERM`。兩者觸發同一 `cancel()`。Windows 僅 Ctrl+C。

## Implementation Contract

**Behavior (operator):**

1. 進程收到 Ctrl+C（或 Unix SIGTERM）→ 開始關機；約 15 秒內退出。
2. 關機期間不再接受新 HTTP 連線；in-flight 請求結束或因 cancel 快速失敗。
3. 正在跑的 agent 子行程被終止（含 process tree）；對應 `run_projects` 成 `failed` 且 error 含 `interrupted by shutdown`。
4. 仍 `queued` 的列保持 `queued`；重啟後可被 worker 接走。
5. 若上次異常退出留下 `running`，下次啟動後那些列變成 `failed` 且 error 含 `interrupted by previous shutdown`，且所屬 run 不再永遠 `running`。

**Interface / data shape:**

- 共用 root `CancellationToken`：存於 `AppState`，傳入 worker／executor／`agent-turn` 路徑。
- 關機 deadline：固定 `Duration::from_secs(15)`。
- Error 字串常數（建議集中）：`interrupted by shutdown`、`interrupted by previous shutdown`。
- HTTP：`axum::serve(...).with_graceful_shutdown(signal_future)`。
- Scheduler：持有可 `shutdown` 的 handle（或等價停止 API），於 cancel 時呼叫。

**Failure modes:**

- `taskkill`／kill 失敗：仍盡力標 DB failed；log error；不阻擋關機繼續。
- Finalize／DB 失敗：log error；關機繼續至 15s 上限。
- Cancel 與 timeout 競態：若 timeout 先到，維持既有 `skipped_timeout`；若 cancel 先到，MUST 為 `failed` + shutdown 字串（不得標成 timeout）。

**Acceptance criteria:**

- 單元／整合測試：啟動補償把 `running` → `failed` + finalize。
- 單元測試：executor 在 token cancel 時呼叫殺樹路徑並回傳 Failed（可用 fake executor／短命子行程）。
- 手動或整合：Ctrl+C 後進程退出且無殘留 agent 子行程（至少在測試替身路徑驗證 cancel→kill）。
- `spectra validate graceful-shutdown` 通過。

**Scope boundaries:**

- In: `lib` run loop、state token、worker、schedule stop、executor select+kill、runs 補償／failed 寫入、`agent-turn` 傳 token、依賴 `tokio-util`（若需要）。
- Out: 前端、新 DB 狀態、可調 timeout env、PID registry、多實例。

## Risks / Trade-offs

- [Risk] 關機時 worker 與 bulk 補償雙寫同一列 → Mitigation: 關機先 cancel token、等 in-flight join（在 15s 內），再可選 bulk 掃殘留 running；啟動補償只在 worker spawn 前。
- [Risk] Windows `taskkill` 權限／時序 → Mitigation: 沿用既有 `kill_process_tree`；失敗只 log。
- [Risk] axum graceful 等 HTTP 與 kill agent-turn 的順序 → Mitigation: 先 cancel token（讓 agent-turn 快失敗），再／同時跑 serve graceful。
- [Trade-off] 15s 硬砍可能截斷未完成 finalize → 可接受；啟動補償兜底。

## Migration Plan

- 無需 DB migration。
- 部署：滾動重啟即可；首次啟動會清掉舊 `running` 殘留。
- Rollback：還原程式碼；殘留行為回到「可能卡住 running」。

## Open Questions

（無 — grill-me 已決議）
