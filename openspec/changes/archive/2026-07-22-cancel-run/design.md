## Context

後端以 `RunWorker` 驅動 run：`drain_queue` 認領 `queued` 的 run_project、spawn 任務、由 executor 起 agent subprocess。取消能力目前只有行程層級一個 `CancellationToken`，存放在 `AppState`，在 shutdown 時被取消一次；executor 各等待點都已對它做 select，因此殺掉 subprocess 的機制已就緒。

缺的是「使用者可觸發、範圍限於單一 run」的入口。現況要停下一個 run 只能重啟行程，代價是所有進行中的專案全被標成 `interrupted by shutdown`。

三個既有事實限制了設計：

1. `runs.status` 的終態是 `success` / `partial` / `failed`，由 `finalize_run_if_complete` 在最後一個專案結束時寫入
2. 認領查詢過濾 `r.status = 'running'`，所以把 run 的 status 改掉就等於關閉該 run 的認領閘門
3. `runs.status` 與 `run_projects.state` 都是無 CHECK 限制的 TEXT，新增終態值不需 migration

## Goals / Non-Goals

**Goals:**

- 使用者能以單一 API 呼叫中止一個進行中的 run，涵蓋執行中與尚未開始的專案
- 被使用者中止與被 shutdown 中斷，在資料上可區辨
- 中止是即時的：執行中的 agent subprocess 會被殺掉，而非等 timeout
- 中止時已寫到磁碟的產出保留並照常 ingest
- 中止不影響其他同時進行的 run

**Non-Goals:**

- 單一專案粒度的中止
- 暫停與續跑
- 刪除已產生的產出
- 修改 per-project timeout 與 `skipped_timeout` 行為
- 中止操作的權限控管

## Decisions

### Per-run cancellation token derived from the shutdown token

`RunWorker` 持有一份 run_id 到 `CancellationToken` 的登錄表。每個 run 的 token 以行程 shutdown token 的 `child_token()` 建立，因此 shutdown 仍會穿透到所有 run，不需要額外的傳播路徑。executor 收到的是 run 的子 token 而非 shutdown token 本身。

登錄項在 run 完結時移除，避免長時間執行的行程累積無用 token。

替代方案：在資料庫放一個 `cancel_requested` 旗標，由 worker 輪詢。否決原因是它把即時取消降級成輪詢延遲，且執行中的 subprocess 仍要等到下一次輪詢才會被殺；既有的 token 機制已經能做到即時。

### Distinguishing user cancellation from process shutdown

子 token 被取消時，worker 無法從 token 本身分辨原因——shutdown 會連帶取消所有子 token。判別方式是檢查行程 shutdown token 是否已取消：若否，則此次取消來自使用者。

- 使用者中止 → `run_projects.state = 'cancelled'`
- shutdown 中斷 → 維持現有的 `state = 'failed'` 加上 `error = 'interrupted by shutdown'`

替代方案：為每個 run 各存一個布林旗標記錄取消來源。否決原因是它與 token 狀態構成兩份可能不一致的真相，而 shutdown token 本身已經是權威來源。

### Cancelled is a terminal state on both runs and run_projects

`run_projects.state` 與 `runs.status` 都新增 `cancelled` 值。尚未開始的專案在中止當下即標為 `cancelled`，不區分「跑到一半被砍」與「根本沒開始」——兩者對使用者的意義相同，都是這批不跑了。

`runs.status = 'cancelled'` 在 API 呼叫當下就寫入，藉此關閉認領閘門，不需要另外的旗標或鎖。

### Finalization must not overwrite a cancelled run

`finalize_run_if_complete` 目前無條件把 run 寫成 `success` / `partial` / `failed`。中止後仍有進行中的專案會陸續結束並呼叫它，若不設防就會把 `cancelled` 蓋掉。完結判定必須在 run 已是 `cancelled` 時直接返回，保留終態。

### Cancelled rows are already terminal at startup recovery

行程重啟時會把前次殘留的 `queued` / `running` 列復原為中斷狀態。`cancelled` 是終態，不在復原範圍內；復原邏輯只針對非終態的列，因此不需為 `cancelled` 加特例，但需以測試釘住這個行為，避免日後改動誤傷。

### Outputs are preserved and ingested

中止不刪除磁碟上的產出，且照常執行 ingest，與 `skipped_timeout` 路徑一致。使用者中止的動機通常包含「保留已經跑出來的部分」。

替代方案：中止時跳過 ingest。否決原因是它讓中止與 timeout 兩條中斷路徑產生無理由的行為分歧，且已寫到磁碟的 review 若不進 DB 就等於不存在，使用者看不到。

## Implementation Contract

**Behavior**

操作者對一個進行中的 run 送出中止請求後：該 run 底下執行中的 agent subprocess 被終止；執行中與尚未開始的專案都落入 `cancelled`；run 本身落入 `cancelled`；已寫到磁碟的產出保留且完成 ingest；其他 run 不受影響。

**Interface**

- `POST /api/runs/{id}/cancel`
- 成功：`200`，回傳該 run 中止後的狀態，形狀與既有 run 詳情端點一致
- run 不存在：`404`
- run 已是終態（`success` / `partial` / `failed` / `cancelled`）：`409`，語意為「無可中止之物」，與既有 `RunConflict` 的用法一致

**Failure modes**

- subprocess 未能在取消後結束：沿用 executor 既有的 kill 路徑，不新增逾時策略
- ingest 於中止路徑失敗：記錄後不中斷中止流程，run 仍落入 `cancelled`——與既有 ingest 失敗不阻斷主流程的處理方式一致
- 中止一個已完結的 run：以 `409` 明確回報，不靜默成功

**Acceptance criteria**

- 中止一個有執行中專案的 run，該 run_project 最終為 `cancelled` 而非 `failed`
- 中止一個同時有 `running` 與 `queued` 專案的 run，兩者最終都是 `cancelled`，且 `queued` 的那個從未被認領執行
- 中止後 `finalize_run_if_complete` 被呼叫，run 狀態仍為 `cancelled`
- 行程 shutdown 造成的中斷仍為 `failed` 加 `interrupted by shutdown`，未被新路徑改變
- 中止 run A 時，同時進行的 run B 不受影響
- 對已完結的 run 送出中止得到 `409`
- 中止時已在磁碟上的產出仍完成 ingest
- 前端在進行中的 run 上顯示中止操作，並正確呈現 `cancelled` 狀態

**Scope boundaries**

- 在範圍內：cancel API 端點、per-run token 登錄表與生命週期、worker 中止路徑的終態判定、完結判定的保護、前端狀態呈現與中止操作
- 不在範圍內：per-project 中止、暫停與續跑、產出刪除、timeout 行為、權限控管、資料庫 schema 變更

## Risks / Trade-offs

- [子 token 被 shutdown 連帶取消，導致 shutdown 期間的中斷被誤判成使用者中止] → 判別以行程 shutdown token 的狀態為準而非子 token；並以測試同時覆蓋 shutdown 路徑與使用者中止路徑，確認兩者落入不同終態
- [`cancelled` 是新的狀態值，前端與後端任一處漏判會顯示錯誤或漏算統計] → 盤點所有消費 run 狀態的位置一併更新；驗收條件明列前端呈現
- [token 登錄表在 run 完結時未移除會造成長期執行行程的記憶體累積] → 移除時機綁在 run 完結路徑上，並以測試確認中止與正常完結兩條路徑都會清掉登錄項
- [中止與最後一個專案自然結束同時發生的競態，可能讓 run 落入非預期終態] → 完結判定在 run 已是 `cancelled` 時直接返回，使兩種順序都收斂到 `cancelled`
- [中止路徑仍執行 ingest，半成品 review 會進到 inbox] → 這是刻意的取捨，與 `skipped_timeout` 一致；使用者若不要半成品，可自行刪除該筆 review
