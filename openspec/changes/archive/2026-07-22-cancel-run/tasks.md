## 1. 取消 token 基礎建設

- [x] 1.1 實作 Per-run cancellation tokens derive from the shutdown token，對應 design 的 Per-run cancellation token derived from the shutdown token：`RunWorker` 持有 run_id 到 `CancellationToken` 的登錄表，每個 token 以行程 shutdown token 的 `child_token()` 建立。驗證：新增 backend/tests/run_cancellation.rs 測試，斷言取消行程 shutdown token 後執行中的 run 觀察到取消，且取消單一 run 的 token 後 shutdown token 與其他 run 的 token 均未被取消。
- [x] 1.2 實作 Run cancellation tokens are released when a run ends：run 進入終態時（無論是被中止或正常完結）從登錄表移除其 token。驗證：測試分別走正常完結與中止兩條路徑，斷言結束後登錄表不再持有該 run_id。
- [x] 1.3 建立 Cancelled is a terminal run state 的狀態常數，對應 design 的 Cancelled is a terminal state on both runs and run_projects：在 runs.rs 定義 `cancelled` 值並確認認領查詢因過濾 `r.status = 'running'` 而自動排除已中止的 run。驗證：測試插入一筆 status 為 `cancelled` 的 run 及其 `queued` 專案，斷言 `fetch_next_queued_run_project` 回傳 `None`。

## 2. 取消傳播與終態判定

- [x] 2.1 實作 Cancellation source determines the terminal state：專案觀察到取消時檢查行程 shutdown token 狀態以判別來源，據此寫入不同終態。這同時滿足 User cancellation is distinguishable from process shutdown，並落實 design 的 Distinguishing user cancellation from process shutdown。驗證：兩個測試分別觸發 shutdown 與使用者中止，斷言前者為 `failed` 且 error 含 `interrupted by shutdown`、後者為 `cancelled`。
- [x] 2.2 實作 Cancellation terminates in-flight project work：中止時取消該 run 的 token，使 executor 殺掉 agent subprocess，該 run_project 落入 `cancelled`，不等 per-project timeout。驗證：以既有測試用的 slow executor 起一個 run，中止後斷言在遠短於 timeout 的時間內該列變為 `cancelled`。
- [x] 2.3 實作 Cancellation prevents queued projects from starting：中止時將該 run 所有 `queued` 專案直接標為 `cancelled`。驗證：測試建立同時含 `running` 與 `queued` 專案的 run，中止後斷言兩者皆為 `cancelled`，且 queued 那筆的 `started_at` 為 null，證明從未被認領執行。
- [x] 2.4 實作 Run finalization preserves cancelled status，對應 design 的 Finalization must not overwrite a cancelled run：`finalize_run_if_complete` 在 run 已是 `cancelled` 時直接返回。驗證：測試先將 run 標為 `cancelled`，再讓殘餘專案完結並觸發完結判定，斷言 run 狀態仍為 `cancelled`。

## 3. Cancel API

- [x] 3.1 實作 Cancel run API endpoint：`POST /api/runs/{id}/cancel` 於成功時回 `200` 與 run 詳情形狀、run 不存在回 `404`、run 已是終態回 `409` 且不更動任何列。驗證：四個 HTTP 測試分別覆蓋 running、不存在、`success`、`cancelled` 四種輸入，並斷言 409 情境下 `run_projects` 狀態未變。
- [x] 3.2 驗證 Cancellation is scoped to one run：中止一個 run 不影響其他同時進行的 run。驗證：測試同時啟動兩個 run，中止其一後斷言另一個的專案繼續執行並到達正常終態。

## 4. 產出保留與重啟復原

- [x] 4.1 實作 Cancellation preserves and ingests produced outputs，對應 design 的 Outputs are preserved and ingested：中止路徑不刪除磁碟產出且照常 ingest，ingest 失敗僅記錄不阻斷中止。驗證：測試在中止前先於磁碟寫入產出，中止後斷言檔案仍存在且已進入資料庫；另一測試令 ingest 失敗，斷言 run 仍到達 `cancelled`。
- [x] 4.2 [P] 驗證 Startup recovery leaves cancelled rows untouched，對應 design 的 Cancelled rows are already terminal at startup recovery：復原邏輯僅針對 `running` 列，`cancelled` 列不得被改動。驗證：測試預先寫入一筆 `cancelled` 列，執行啟動復原後斷言其 state 與 error 均未變。

## 5. 前端呈現與操作

- [x] 5.1 [P] 實作 Run history surfaces cancelled status：執行歷史清單與 run 詳情將 `cancelled` 呈現為有別於 `failed` 的狀態，專案列亦同。驗證：RunsPage.test.tsx 新增測試，渲染含 `cancelled` run 與專案的資料，斷言畫面出現中止樣式而非失敗樣式。
- [x] 5.2 實作 Run history offers cancellation for in-progress runs：`running` 的 run 顯示中止操作、終態 run 不顯示，觸發後呼叫 cancel 端點並在無需手動重整的情況下反映新狀態。驗證：RunsPage.test.tsx 三個測試分別覆蓋 running 顯示操作、終態不顯示、觸發成功後狀態更新為 `cancelled`。

## 6. 整體驗證

- [x] 6.1 確認全案無回歸：後端 `cargo test -p reviewer-server` 全綠、`cargo clippy --all-targets` 無新增警告、前端測試全綠。驗證：逐項執行並記錄結果；已知既有失敗 `mr_scan_timeout_still_ingests_draft_on_disk` 與 app-env crate 的三個測試不計入，但須確認其失敗訊息與本變更前相同。
