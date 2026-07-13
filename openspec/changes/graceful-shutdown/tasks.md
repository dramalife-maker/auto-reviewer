## 1. 依賴與常數

- [x] 1.1 依 Decision: CancellationToken 傳遞取消，在 `backend/Cargo.toml` 加入 `tokio-util`（含 `CancellationToken` 所需 feature）。驗證：`cargo check -p reviewer-server` 成功解析依賴。
- [x] [P] 1.2 集中定義 error 字串常數 `interrupted by shutdown` 與 `interrupted by previous shutdown`（對齊 Requirement: Interrupted running projects are marked failed 與 Requirement: Startup recovers orphaned running projects）。驗證：常數可被 `runs`／worker／executor 模組引用編譯通過。

## 2. 啟動補償（TDD）

- [x] 2.1 先寫失敗測試：DB 預置一筆 `run_projects.state='running'` 與父 run `status='running'`，呼叫啟動補償後列變成 `failed` 且 error 含 `interrupted by previous shutdown`，父 run 被 finalize（對齊 Requirement: Startup recovers orphaned running projects 與 Decision: 啟動補償殘留 running）。驗證：測試先紅。
- [x] 2.2 實作啟動補償並於 worker spawn 前呼叫，使 2.1 變綠。驗證：該測試通過；`queued` 列不被改動。

## 3. Executor 取消（TDD）

- [x] [P] 3.1 先寫失敗測試：spawn 長跑子行程（或 test executor），對 CancellationToken `cancel()` 後 `execute_weekly_batch`／`execute_mr_review` 回傳 Failed（非 SkippedTimeout），error 識別 shutdown（對齊 Requirement: Reviewer executors honor shutdown cancellation、Requirement: In-flight reviewer subprocesses are cancelled and killed、Decision: CancellationToken 傳遞取消）。驗證：測試先紅。
- [x] 3.2 實作 executor `select!` wait vs cancel，cancel 時 `kill_process_tree` 並回傳 Failed；`execute_agent_turn` 同樣接受 token。驗證：3.1 變綠；既有 timeout → `skipped_timeout` 測試仍通過（cancel 與 timeout 語意不混淆）。

## 4. Worker／Scheduler 停收新工作

- [x] 4.1 Worker 持有 root token：cancelled 後不再 dequeue；in-flight job 因 executor cancel 走 failed + `interrupted by shutdown` 並 finalize（對齊 Requirement: Coordinated shutdown stops HTTP, scheduler, and new worker jobs、Requirement: Interrupted running projects are marked failed、Decision: 中斷終態為 failed、Decision: 全層關機（HTTP + worker／scheduler + kill 子行程））。驗證：整合或單元測試——cancel 後 drain 不再取新 queued；被中斷的 running 成 failed。
- [x] [P] 4.2 Scheduler 可在 shutdown 時停止，不再 enqueue cron runs（對齊 Requirement: Coordinated shutdown stops HTTP, scheduler, and new worker jobs）。驗證：單元／整合測試 mock 或短週期 cron，cancel 後不再產生新 `runs` 列。

## 5. 信號、HTTP graceful、15s 上限

- [x] 5.1 實作 Ctrl+C + Unix SIGTERM → 同一 `cancel()`（對齊 Requirement: Process shutdown is triggered by Ctrl+C and Unix SIGTERM、Decision: 信號為 Ctrl+C + Unix SIGTERM）。驗證：編譯期 `cfg(unix)`／Windows 路徑皆過；文件或測試註明 Windows 僅 Ctrl+C。
- [x] 5.2 `axum::serve` 接 `with_graceful_shutdown`；`AppState` 暴露 token 給 `agent-turn`；關機自信號起硬上限 15 秒（對齊 Requirement: Coordinated shutdown stops HTTP, scheduler, and new worker jobs、Decision: 固定 15 秒關機硬上限、Decision: 全層關機（HTTP + worker／scheduler + kill 子行程））。驗證：手動或整合——啟動 server、觸發 shutdown future，進程在 15 秒內結束且 HTTP 停收新連線。

## 6. 回歸與規格對齊

- [x] 6.1 跑後端相關測試套件確認無回歸。驗證：`cargo test -p reviewer-server` 通過。
- [x] 6.2 對照 specs 逐條確認 Scenario 有對應測試或手動驗證紀錄。驗證：`spectra validate graceful-shutdown` 通過。
