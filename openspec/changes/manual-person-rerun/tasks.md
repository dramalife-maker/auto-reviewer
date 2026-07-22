## 1. Schema

- [x] 1.1 新增 migration `backend/migrations/015_run_projects_person.sql`，對 `run_projects` 加 nullable 欄位 `person_id INTEGER REFERENCES people(id)`（實作 Requirement: run_projects carries an optional person scope 的 schema 部分）。行為：既有列自動為 NULL（＝整批語意），無需回填。驗證：`cargo test` 啟動時套用 migration 不報錯；新欄位存在且可為 NULL。

## 2. 後端 run 建立與 job 串接（runs.rs）

- [x] 2.1 在 `backend/src/runs.rs` 的 `RunProjectRow` 加 `person_id: Option<i64>`，並在 `fetch_next_queued_run_project` 的 readback SELECT 補 `rp.person_id`（不動原子 claim UPDATE）（完成 Requirement: run_projects carries an optional person scope 的 claim 曝露部分）。行為：worker 取出的 job 帶 person 範圍。驗證：新增測試斷言由 `manual_person` run 建立的 run_project 被 claim 後 `person_id` 為 Some(該人)。
- [x] 2.2 在 `backend/src/runs.rs` 新增 `create_manual_person_run(pool, project_name, person_id) -> Result<i64>`：全系統閘 `has_active_run_projects`（true→`RunConflict`）、project by name 查無→`NotFound`、person by id 查無→`NotFound`，交易內 `INSERT runs(trigger='manual_person', status='running', project_total=1)` + `INSERT run_projects(run_id, project_id, person_id, state='queued')`（實作 Requirement: Manual single-person run enqueues one project scoped to one person 的 run 建立部分）。行為：建立單人 run 與單一 queued run_project 帶 person_id。驗證：新增測試涵蓋 happy path、409 衝突、project 404、person 404。

## 3. 週報 manifest 依 person 過濾（runs.rs）

- [x] 3.1 在 `backend/src/runs.rs` 的 `write_weekly_manifest` 加參數 `person_id: Option<i64>`；`Some(pid)` 時先查該人 `people.display_name`，於函式尾端過濾：`authors.retain(person_id==pid)`、`open_pending.retain(person_id==pid)`、`published_pending_snippets.retain(路徑首段==display_name)`；`None` 維持現行整批內容（實作 Requirement: Weekly manifest is filtered to the run project person scope）。行為：單人 run 的 manifest 三塊僅含該人；整批 run 內容不變。驗證：新增測試給定兩人資料，`Some(1)` 產出三塊皆只剩 person 1；`None` 產出含全部；`Some` 指向無窗口活動者產出空 `authors` 且不報錯。

## 4. worker/executor 串接 person 範圍

- [x] 4.1 在 `backend/src/worker.rs` 與 `backend/src/executor.rs` 將 `job.person_id` 一路串到 `execute_weekly_batch` → `write_weekly_manifest`（銜接 Requirement: Weekly manifest is filtered to the run project person scope 與 job claim）。MR 軌道路徑（`process_mr_run_project`）不讀 person_id。行為：weekly job 的 person 範圍傳達到 manifest 產生點；MR 路徑不受影響。驗證：`cargo build` 通過；既有 weekly 與 MR 執行測試全綠（`person_id=None` 零回歸）。

## 5. HTTP 介面（server.rs）

- [x] 5.1 在 `backend/src/server.rs` 的 `CreateRunRequest` 加 `person_id: Option<i64>`，並於 `create_run` 新增 `"manual_person"` 分支：缺 `project_name` 或 `person_id`→400，否則呼叫 `runs::create_manual_person_run`（完成 Requirement: Manual single-person run enqueues one project scoped to one person 的 HTTP 契約：201/400/404/409）。行為：`POST /api/runs {trigger:"manual_person",...}` 依契約回應。驗證：新增測試涵蓋缺欄位 400 與 happy path 201。

## 6. 前端顯示

- [x] 6.1 [P] 在 `frontend/src/lib/format.ts` 的 `humanizeTrigger` 加 `case 'manual_person': return '手動單人'`。行為：run 歷史列表顯示中文標籤而非原始 trigger 字串。驗證：`humanizeTrigger('manual_person')` 回傳 `'手動單人'`（若有前端測試則加一例，否則 build 通過即可）。

## 7. 驗證與回歸

- [x] 7.1 執行 `cargo test`（backend）確認：新測試（2.1、2.2、3.1、5.1）全綠、既有 `backend/tests/runs_execution.rs` 整批路徑測試全綠。行為：整批行為零回歸、單人路徑符合契約。驗證：測試全數通過。
