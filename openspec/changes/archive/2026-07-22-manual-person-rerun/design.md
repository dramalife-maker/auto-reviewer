## Context

手動週報觸發目前只有兩個粒度：`manual_all`（全專案全人）與 `manual_project`（單專案全人）。實務上管理者常在補完一位工程師的歸戶資料後想單獨驗證，卻被迫重跑整個專案，造成其他工程師被重複處理（多餘 LLM 呼叫、side effect）。

已查證的現況約束：
- 週報 workflow（`skills/reviewer-batch/WORKFLOW.md`）完全以 person 為單位輸出，所有檔案寫在 `{report_root}/{person}/` 或 `{person_report_root}/{display_name}/`，**無任何跨人的專案級彙總檔**；`notes_dir`（`.notes` ADR）在 headless 為唯讀。因此把 manifest 過濾成單一 person，整個 run 即乾淨隔離。
- 週報 manifest 由 `write_weekly_manifest`（backend/src/runs.rs）產生，內含三塊帶 person 身分的資料：`authors`（`ManifestAuthor.person_id`）、`open_pending`（`ManifestOpenPending.person_id`）、`published_pending_snippets`（`Vec<String>` 相對路徑，首段為 `display_name` 資料夾）。
- worker 併發不變式（原子 claim `fetch_next_queued_run_project`、drain tick、`has_active_run_projects` 全系統閘）為近期以大量測試穩定的成果。
- `people` 為全域表，無 `project_id`；person 是否屬於某專案只能靠 `person_identities` 在該 repo 是否出現間接判斷。
- issue #3（`pending_items` 重複插入）已於 `pending-replay-dedup` 修復，本變更的重複插入曝險前提已解除。

## Goals / Non-Goals

**Goals:**

- 新增 `manual_person` trigger，讓 `POST /api/runs` 可只重算某工程師在某專案的週報。
- 單人 run 對其他工程師與整批行為零副作用、零回歸。
- 重用既有 worker 併發／claim／finalize／cancel／recovery 機制，不改動其不變式。

**Non-Goals:**

- 不放寬併發模型（不引入單專案閘或 per-person 閘）。
- 不在 API 端驗證 person 是否屬於專案或窗口內是否有 commit。
- 前端入口只放「人員設定」頁的「參與專案」清單；不在 Dashboard／Projects／Runs 等頁另加人員選擇器或觸發點。
- 不改動 ingest／finalize／cancel／recovery 邏輯。

## Decisions

**D1 — 併發閘用全系統 `has_active_run_projects`（Q1=a）。**
替代方案：(b) 單專案閘 `has_active_run_for_project`、(c) per-person 閘。選 (a) 因單人重跑屬低頻手動驗證操作、非吞吐瓶頸；放寬併發會讓「一個 run 混不同 gating 語意」，擴大剛穩定的併發模型測試面與 race 風險。issue #4 的價值靠 manifest 過濾即達成，與併發閘寬鬆無關。放寬併發應為獨立議題。

**D2 — API 僅驗存在性（Q2=a）。**
替代方案：額外驗歸屬（要求有 `person_identities` 或窗口內有 commit）。選只驗存在性，因 WORKFLOW 對 `authors` 為空已有安全處理（正常結束、非錯誤），空 run 天然 no-op；準確判斷「窗口有無 commit」需先跑 git log，等於把 worker 工作塞進 API 同步路徑，違反 surgical。呼叫端傳錯 person_id 只會產出空 run，不留髒資料。

**D3 — person 過濾放在 `write_weekly_manifest`，新增 `person_id: Option<i64>` 參數。**
`None` = 現行整批行為（零回歸）。`Some(pid)` 時在函式尾端過濾三塊：`authors.retain(person_id == pid)`、`open_pending.retain(person_id == pid)`、`published_pending_snippets.retain(首段路徑 == 該人 display_name)`。需先查該人 `display_name`（也兼作 person 存在確認）。替代方案：把 person filter 下推進 `prepare_manifest_authors` / `load_*`——否決，因尾端 retain 最 surgical，三塊資料本就帶 person 身分，底層 query 完全不動。

**D4 — `run_projects` 加可為 NULL 的 `person_id`。**
migration `015_run_projects_person.sql`：`ALTER TABLE run_projects ADD COLUMN person_id INTEGER REFERENCES people(id)`。NULL = 整批（沿用語意）。`RunProjectRow` 加 `person_id: Option<i64>`；`fetch_next_queued_run_project` readback SELECT 補 `rp.person_id`（原子 claim UPDATE 邏輯不動）。job → `execute_weekly_batch` → `write_weekly_manifest` 串下去。替代方案：另建 side table 標記單人 run——否決，過度設計；nullable 欄位語意最直接。

**D5 — `create_manual_person_run(pool, project_name, person_id)`。**
比照 `create_manual_project_run` 結構：全系統閘（D1）→ project by name 查無 404 → person by id 查無 404 →交易內 `INSERT runs(trigger='manual_person', status='running', project_total=1)` + `INSERT run_projects(run_id, project_id, person_id, state='queued')`。

**D6 — server `create_run` 新增 `manual_person` 分支。**
`CreateRunRequest` 加 `person_id: Option<i64>`；分支解析 `project_name` + `person_id`，任一缺→400（比照 `manual_project` 缺 `project_name` 的 400 慣例）。

**D7 — MR 軌道不受影響。**
`is_mr_trigger("manual_person")` 為 false，走 weekly 路徑。MR run 的 `run_projects.person_id` 恆 NULL，`process_mr_run_project` 不讀該欄位。

**D8 — 前端補標籤。**
`humanizeTrigger` 加 `case 'manual_person': return '手動單人'`。`default` 已能容錯，此為顯示品質補強。

**D9 — 前端觸發入口放「人員設定」頁的「參與專案」清單（就地驗證）。**
情境：管理者在人員設定頁修完某人的 identity 綁定後，想立刻驗證。該頁 `PersonDetail.projects`（`GET /api/people/{id}` 已回傳，型別 `PersonProjectItem { id, name }`）正好同時具備 `person_id`（選中人員）與 `project_name`，是唯一天然同時握有兩者的位置。作法：`api.ts` 加 `startPersonRun(projectName, personId)`；「參與專案」每個 project 後加「重跑週報」按鈕 → 呼叫後成功／失敗都走既有 `useToast`。409（全系統閘擋下）照實把後端訊息 toast 出來，不另做前端併發判斷。替代方案：Dashboard 或 Projects 頁加人員選擇器——否決，那些頁沒有現成的「人＋專案」配對，需額外拉清單與選擇器，範圍與 UX 都更重，且偏離「修完歸戶就地驗證」情境。

## Implementation Contract

**Behavior：** 呼叫 `POST /api/runs` 帶 `{ "trigger": "manual_person", "project_name": "<name>", "person_id": <id> }` 時，後端建立一筆 `runs`（`trigger='manual_person'`、`status='running'`、`project_total=1`）與一筆 `run_projects`（`state='queued'`、`person_id=<id>`），回 `201` + `{ "run_id": <i64> }`。worker 取出後產生的週報 manifest 只含該 person 的 `authors` / `open_pending` / `published_pending_snippets`，headless agent 只在該 person 目錄產出／消費，不觸及他人。

**Interface / data shape：**
- Request：`CreateRunRequest { trigger: String, project_name: Option<String>, person_id: Option<i64> }`。
- `run_projects` 新欄位 `person_id INTEGER NULL REFERENCES people(id)`（NULL = 整批）。
- `RunProjectRow.person_id: Option<i64>`。
- `write_weekly_manifest(pool, data_root, run_id, project, repo_path, person_id: Option<i64>)`。
- manifest JSON schema 不變（仍是既有 `authors` / `open_pending` / `published_pending_snippets`，只是內容被過濾）。

**Failure modes：**
- 系統有任何 active run → `409`（`RunConflict`，沿用既有）。
- `project_name` 或 `person_id` 缺 → `400`。
- project 查無 → `404`；person 查無 → `404`。
- person 存在但窗口內無 commit／無 pending → run 正常完成、輸出為空（非錯誤，沿用 `authors` 空陣列語意）。

**Acceptance criteria：**
- `backend/tests/runs_execution.rs` 新增：`create_manual_person_run` happy path（run + run_project 帶 person_id）、併發衝突 409、project 404、person 404。
- manifest 過濾測試：給定兩位 author／兩人 open_pending／兩人 snippets，帶 `person_id` 產生的 manifest 三塊皆只剩目標 person。
- `fetch_next_queued_run_project` 回傳含 `person_id`。
- server 層：`manual_person` 缺 `project_name` 或 `person_id` → 400；happy path → 201。
- 既有整批測試（`person_id = None` 路徑）全綠，證明零回歸。

**Scope boundaries：**
- In scope：上述 backend 5 檔 + migration + 前端 `humanizeTrigger` 一行 + 前端單人觸發入口（`api.ts` 的 `startPersonRun` + `PeoplePage` 「參與專案」重跑按鈕與 handler）+ 測試。
- Out of scope：Dashboard／Projects／Runs 等頁的人員選擇器或觸發點、併發模型放寬、歸屬驗證、ingest／cancel／recovery 邏輯改動。

## Risks / Trade-offs

- [傳錯 person_id 安靜產出空 run] → 可接受；run 記錄 `project_total=1`、輸出為空、不留髒資料。若日後需要，可在 response 附「窗口命中 authors 數」提示，屬加分項不納入本變更。
- [readback SELECT 改動觸及原子 claim readback] → 僅新增一個欄位到既有 readback query，不動 claim UPDATE；以既有 claim 測試 + 新 person_id 回傳測試覆蓋。
- [migration 對既有 `run_projects` 列] → 新欄位 nullable 無預設，既有列自動為 NULL＝整批語意，無需回填。

## Migration Plan

1. 套用 migration `015_run_projects_person.sql`（新增 nullable 欄位，既有列自動 NULL）。
2. 後端向前相容：`manual_all` / `manual_project` / MR 路徑皆走 `person_id = None`，行為不變。
3. 無資料回填、無破壞性變更、無需協調前後端上線順序（前端標籤為純顯示增益）。
