## Context

軌道 1（週報批次）已上線：`backend/src/schedule.rs` 用 `tokio-cron-scheduler` 註冊單一每週 cron，`backend/src/runs.rs` 提供 `manual_all`/`manual_project` 兩種手動觸發，去重鎖靠 `run_projects` 表（同 `project_id` 若已有 `state IN ('queued','running')` 的列即拒絕新排入，HTTP 409）。`backend/src/worktree.rs` 的 `provision_all`/`provision_project` 已對每個有 `git_remote_url` 的專案做 bare-clone + resident worktree 供給，且冪等。

`mr_reviews` 表與 `schedule_config.mr_poll_interval_min` 欄位已存在於 `backend/migrations/001_initial.sql`，但完全未被使用：沒有 `mr_reviews.rs` 模組、沒有第二個 cron job、`runs.trigger` 目前只接受 `manual_all`/`manual_project` 兩個字面值。`skills/` 目錄只有 `reviewer-batch/`，沒有 `scan-mrs-headless/`。

參考：`docs/idea/spec.md §6.1/§6.4/§6.5/§7`、`docs/idea/schema.md §5.1`、既有互動式 plugin skill `cto:scan-mrs`（僅供邏輯參考，不在此 repo 內，雲端不部署該 plugin）。

## Goals / Non-Goals

**Goals:**
- 讓軌道 2（MR review）從「schema 已預留但完全未跑」變成「排程輪詢 + 單專案手動觸發都能實際產出 `mr_reviews` 草稿」。
- 草稿產出到人工發佈之間全程唯讀（不寫 GitLab），發佈才代跑 `glab mr note`。
- 高頻輪詢下 token 成本可控：MR 列舉／分輪／去重由 **triage script**（`glab` + 確定性規則）在啟動 agent 前完成；agent 只處理已篩選出的 MR。
- 軌道 1 與軌道 2 共用同一把 per-project 去重鎖與同一個 worker pool，避免同專案被兩軌同時 clone/checkout。
- 每份 MR 草稿綁定產出時的 agent `session_id`，管理者可透過 API 接回同一 session 追問 AI 的判斷依據。

**Non-Goals:**
- 不做自動漏跑偵測補跑（見 proposal Non-Goals）。
- 不做多使用者認證／per-manager 發佈身分（見 proposal Non-Goals）。
- 不做前端收件匣 UI（本次僅後端 API）。
- 不建立獨立的第二套 clone/fetch 系統；MR worktree 建立擴充既有 `repo-worktree` 能力，沿用其 bare repo 與 remote 設定。

## Decisions

### `runs.trigger` 擴充為四個合法值

新增 `mr_poll`（排程觸發）與 `manual_mr_poll`（單專案手動觸發），與既有 `schedule`/`manual_all`/`manual_project` 並列。理由：`runs`/`run_projects` 的去重鎖、狀態機、`run_projects.state` 生命週期已經是成熟模式，MR 掃描直接復用比另建一套追蹤表便宜。四個值的觸發-範圍對應：

| trigger | 範圍 | 入口 |
|---|---|---|
| `schedule` | 全部健康專案 | 週報 cron |
| `manual_all` | 全部專案 | 控制台「全部執行」 |
| `manual_project` | 單一專案 | 專案清單 hover「▶ 執行」 |
| `mr_poll` | 全部健康專案 | MR 輪詢 cron（新增） |
| `manual_mr_poll` | 單一專案 | 專案清單「▶ 掃 MR」（新增） |

替代方案（否決）：為 MR 軌道另建 `mr_runs`/`mr_run_projects` 平行表。否決理由：兩軌的執行狀態機（queued/running/done/skipped_timeout/failed）完全一致，另建表會重複去重鎖邏輯且讓「同專案是否正在被任一軌道佔用」的查詢變成要 UNION 兩張表。

### MR 輪詢 cron 為獨立第二個 job，而非合併進週報 cron

`start_scheduler` 新增第二個 `Job::new_async_tz`，週期依 `schedule_config.mr_poll_interval_min`（分鐘）換算 cron 表達式，對所有 `is_git_repo=1` 專案建立 `trigger='mr_poll'` 的 run。理由：週期單位不同（週報用 weekday+run_time，MR 輪詢用固定分鐘間隔），合併會讓 `build_cron_expression` 邏輯分岔處理兩種語意，拆開更直觀。

### 去重鎖規則擴大到跨軌道

現有 `run_projects` 插入前檢查「同 `project_id` 是否已有 `state IN ('queued','running')` 的列」——此檢查**不分 `run_id` 所屬的 `trigger`**，因此兩軌天然共用同一把鎖，不需修改鎖邏輯本身，只需確認新增的 `mr_poll`/`manual_mr_poll` run 走同一段插入程式碼路徑（`create_batch_run`/`create_manual_project_run` 的既有模式）。

### MR worktree 為 on-demand 建立，擴充既有 `repo-worktree` 能力而非新建系統

`repo-worktree` spec 已定義 merge-request worktree 的命名規則（`escape_branch` 對非 `[A-Za-z0-9._-]` 字元跳脫，並對 MR worktree 加上分支名短 hash 後綴避免碰撞），但目前沒有任何呼叫端會實際 fetch MR 來源分支並建立該 worktree——只有命名工具，沒有 orchestration。本次在 `worktree.rs` 新增 `provision_mr_worktree(repo_path, source_branch) -> PathBuf`：對既有 `.bare/` 執行 `git fetch origin <source_branch>`，再依 `escape_branch` 規則建立（或重用，若已存在）worktree。多個 MR 共用同一來源分支時，重用同一個 worktree 目錄（spec 既有規則：「Multiple merge requests that share the same source branch MUST map to the same merge-request worktree directory」）。

否決方案：在 `mr-review` 能力內另建一套獨立的 fetch/worktree 邏輯。否決理由——`repo-worktree` 已經為此設計了命名規則與碰撞測試（`mr_names_disambiguate_escape_collisions`），只是缺 orchestration 函式；由 `mr-review` 另建等於繞過既有能力邊界、造成兩套 worktree 管理邏輯並存。

### headless workflow 產出檔案，後端解析入庫（沿用軌道 1 的邊界原則）

`scan-mrs-headless/WORKFLOW.md` 只碰檔案與**單一已選定 MR** 的 review：讀 manifest 內該 MR 的 metadata（由 triage script 預填）、在 MR worktree 內檢視 diff／程式碼、寫草稿 md 到 manifest 指定的 `draft_dir`、寫觀察片段到 `_pending/`。不查 SQLite、不執行 `glab mr list`／MR 篩選、不執行 `glab mr note`/`glab mr merge`。後端在子行程結束後解析 `draft_dir` 下的草稿檔（frontmatter 含 `mr_iid`/`mr_title`/`review_round`/`author_identity`），upsert 進 `mr_reviews`（`UNIQUE(project_id, mr_iid, review_round)` 已存在於 schema，重複解析會是 upsert 而非新增列）。

### MR triage script 負責列舉與篩選，不經 AI agent

現有 `cto:scan-mrs` 的「列 open MR → 判斷第一輪/第二輪 → 依 GitLab notes `By: AI Agent` 去重」若全交給 agent 透過 tool call 執行，每輪輪詢都會消耗大量 token 在機械式 `glab mr list`／`glab mr view` 上。

**決策**：新增 `scripts/triage-mrs.py`，由後端在每次 MR 掃描（`mr_poll`／`manual_mr_poll`）啟動任何 agent 子行程**之前**執行：

1. 在專案 resident worktree（`cwd`）呼叫 `glab mr list`／`glab mr view`（JSON 輸出），取得 open MR 清單與 notes。
2. 對每個 MR 套用確定性規則（移植自 `cto:scan-mrs`／`spec.md §6.4`）：
   - **第一輪**（`review_round=1`）：MR 上尚無含 `By: AI Agent` 標記的 note。
   - **第二輪**（`review_round=2`）：已有 AI Agent note，但 MR 自該 note 後有新 commit 或新 discussion（無新動靜則跳過）。
   - **跳過**：已有 AI Agent note 且判定無新動靜。
3. 將結果寫入 manifest 同目錄的 `eligible_mrs.json`：

```json
{
  "generated_at": "2026-07-09T08:00:00Z",
  "eligible": [
    {
      "mr_iid": 42,
      "mr_title": "feat: add cache",
      "source_branch": "feature/cache",
      "author_identity": "alice@co.com",
      "review_round": 1
    }
  ],
  "skipped": [
    { "mr_iid": 7, "skip_reason": "no_new_activity_since_ai_note" },
    { "mr_iid": 9, "skip_reason": "gitlab_draft" },
    { "mr_iid": 11, "skip_reason": "label:wip" },
    { "mr_iid": 15, "skip_reason": "missing_required_label:ready-for-review" }
  ]
}
```

4. 後端讀取 `eligible` 陣列，**僅對其中 MR** 依序 `provision_mr_worktree` + spawn agent。`eligible` 為空時，標記 `run_projects.state='done'` 且不啟動 agent（零 token 掃描）。

### MR review 防呆：排除「尚未準備好」的 MR

工程師常有尚未準備好、不希望 AI 提前 review 的 MR。此判斷屬**確定性規則**，應在 triage script 完成，不交由 agent。

**三層防呆（由嚴到鬆，可疊加）**：

| 機制 | 誰操作 | triage 行為 | `skip_reason` |
|------|--------|-------------|---------------|
| **GitLab Draft MR** | 作者在 GitLab 勾選 Draft | 一律跳過 | `gitlab_draft` |
| **排除標籤** | 作者加 label（如 `wip`、`do-not-review`） | 命中任一 `mr_review_skip_labels` 即跳過 | `label:<name>` |
| **必備標籤（可選）** | 作者準備好後加 `ready-for-review` | 專案設了 `mr_review_require_label` 時，MR **必須**帶該 label 才進 `eligible` | `missing_required_label:<name>` |

**專案級設定**（寫入 `projects` 表，manifest 帶給 triage script）：
- `mr_review_skip_labels`：JSON 字串陣列，預設 `["wip", "do-not-review", "no-ai-review"]`。專案可覆寫或設為 `[]` 關閉標籤排除（Draft MR 仍跳過）。
- `mr_review_require_label`：可為 `NULL`（不啟用 opt-in 模式）。設為 `"ready-for-review"` 時，只有帶此 label 的 MR 才會被 review——適合「明確標記才請 AI 看」的團隊。

**推薦工作流（給工程師）**：
1. 開 MR 時保持 **Draft** 或加 **`wip`** label → AI 不會掃。
2. 準備好後：取消 Draft、移除 `wip`；若專案啟用 require label，加上 **`ready-for-review`**。
3. 下一輪 MR 輪詢 triage 通過 → agent 產草稿。

**替代方案（否決）**：在 SQLite 建 per-MR 手動封鎖表 + 管理 UI。否決理由：增加維護面；GitLab Draft／label 是工程師已有習慣、且 MR 上可見的訊號，不需額外後台操作。若未來有「不改 GitLab 也要擋」需求，可另開 change 加 `mr_review_blocks` 表。

**替代方案（否決）**：靠 MR title 前綴 `[WIP]` 判斷。否決理由：易漏、易誤判、無法在 GitLab UI 篩選。

**邊界**：
- Script 只讀 GitLab（`glab`），不碰 SQLite、不呼叫 AI。
- Script 失敗（`glab` 錯誤、JSON 解析失敗）→ 整個 `run_project` 標 `failed`，不 spawn agent。
- Agent workflow **不得**再執行 `glab mr list` 做 MR 發現；允許對**已指定**的 `mr_iid` 執行 `glab mr diff`／`glab mr view` 取得 review 素材（這是 review 本體，非 triage）。

**替代方案（否決）**：維持由 agent workflow 自行 `glab mr list` 篩選。否決理由：高頻輪詢下重複 tool call 燒 token，且篩選邏輯為確定性規則，不需要 LLM 推理。

**替代方案（否決）**：把 triage 邏輯寫進 Rust 後端。否決理由：`glab` 輸出格式與 scan-mrs 分輪規則較適合獨立 script 迭代與單元測試；Rust 僅負責 orchestration（執行 script → 讀 JSON → spawn agent）。

### MR 掃描採「每個 MR 一個 agent 子行程」，並持久化 session

週報軌道（`execute_weekly_batch`）刻意使用 `--no-session-persistence`，因為批次產報不需事後追問。MR 草稿則相反：管理者可能對單一 MR 的 review 判斷有疑慮，需要接回**當初產出該草稿的 agent session** 繼續對話。

**決策**：
- 掃描 orchestration 對每個待 review 的 MR 各自 spawn 一個 headless 子行程（manifest 帶 `mr_iid`），而非一次子行程處理整個專案所有 MR。理由：session 是 provider-owned conversation state（Claude `--resume <id>`、Cursor `--resume <chatId>`），一個 session 對應一份草稿，接續對話時上下文不會混入其他 MR。
- MR 子行程**不**傳 `--no-session-persistence`（Claude）／等同啟用 session（Cursor 預設即持久化）。
- 子行程使用 `--output-format stream-json`；後端在子行程結束後解析 stdout NDJSON，擷取 `session_id`：
  - Claude：`result` 事件或 `--output-format json` 的 `.session_id`
  - Cursor：`system/init.session_id` 或 `result.session_id`（見 `cursor-agent-cli.md` contract）
- 擷取到的 `session_id` 與當次使用的 `reviewer_agent`（`claude`|`cursor`，沿用 `REVIEWER_AGENT`）一併寫入 `mr_reviews.agent_session_id` / `mr_reviews.reviewer_agent`。
- 若子行程成功產出草稿但 stdout 解析不到 `session_id`，草稿仍入庫，`agent_session_id` 留 `NULL`；`agent-turn` API 對此類列回 `409`（無法接續）。

**替代方案（否決）**：整個專案掃描共用一個子行程、所有草稿共用同一 `session_id`。否決理由：接續對話時上下文會包含同批次其他 MR，管理者追問「為什麼 MR #42 這樣寫」時 agent 可能被其他 MR 內容干擾。

**替代方案（否決）**：由 workflow 在草稿 frontmatter 自行寫入 `session_id`。否決理由：workflow 不應知道 CLI session 契約；session 由後端從 subprocess stdout 擷取才是單一真相來源。

### 草稿接續對話 API

`POST /api/mr-reviews/:id/agent-turn` 接受 `{ "message": string }`，後端以該列的 `reviewer_agent` + `agent_session_id` 啟動 headless 子行程（`claude -p <message> --resume <id>` 或 `cursor-agent --print --resume <id> <message>`），回傳 `{ "reply": string, "agent_session_id": string }`。僅 `status='draft'` 且 `agent_session_id IS NOT NULL` 時允許。此 API 不修改 `draft_md_path`、不觸發 GitLab；管理者確認後仍須手動編輯草稿或發佈。

## Implementation Contract

**行為（Behavior）**：
- 排程輪詢：服務啟動後，每 `schedule_config.mr_poll_interval_min` 分鐘，對每個 `is_git_repo=1` 的專案建立一個 `runs.trigger='mr_poll'` 的執行。每個專案掃描先跑 `triage-mrs.py` 產出 `eligible_mrs.json`，再僅對 eligible MR 啟動 agent，結果落地為 `mr_reviews` 列（`status='draft'`）。
- 手動觸發：呼叫 `POST /api/projects/:id/mr-scan` 對單一專案立即建立 `runs.trigger='manual_mr_poll'` 的執行，行為與排程輪詢對該專案的單次掃描完全相同，差別只在觸發來源與是否受 `mr_poll_interval_min` 週期限制（手動觸發不受週期限制，但仍受去重鎖限制——若該專案已有 `queued`/`running` 的執行，回 409）。
- 收件匣：`GET /api/mr-reviews?status=draft` 回傳待處理草稿列表（含 `agent_session_id`、`reviewer_agent`）；`POST /api/mr-reviews/:id/publish` 呼叫 `glab mr note <mr_iid> --message <body>`（`cwd`=該專案的 resident worktree，即 `default_branches` 第一個分支的 worktree；發佈只需 `glab` 能解析出對應的 GitLab 專案，不需要 MR 專屬 worktree），成功後更新 `status='published'`、`published_at`、`published_body`；失敗回 502 並保留 `status='draft'`。`POST /api/mr-reviews/:id/ignore` 更新 `status='ignored'`，不呼叫 GitLab。`POST /api/mr-reviews/:id/agent-turn` 以儲存的 session 接續 headless 對話（見上方決策）。

**介面 / 資料形狀**：
- `mr_reviews` 表新增欄位（migration `002_mr_review_agent_session.sql`）：
  - `agent_session_id TEXT` — provider-owned session token；`NULL` 表示該草稿無法接續對話
  - `reviewer_agent TEXT NOT NULL DEFAULT 'cursor'` — 產出／接續時使用的 CLI（`claude`|`cursor`）
- 新增 API：
  - `POST /api/projects/:id/mr-scan` → `202 { "run_id": <i64> }`；若去重鎖擋下 → `409`。
  - `GET /api/mr-reviews?status=draft|published|ignored`（預設 `draft`）→ `200 [{ id, project_id, project_name, person_id, author_name, mr_iid, mr_title, review_round, status, draft_body, agent_session_id, reviewer_agent, created_at }]`。
  - `PATCH /api/mr-reviews/:id { "draft_body": string }` → 更新草稿內容（不觸發 GitLab），`200`。
  - `POST /api/mr-reviews/:id/publish` → `200 { published_at, published_body }` 或 `502 { error }`。
  - `POST /api/mr-reviews/:id/ignore` → `200`。
  - `POST /api/mr-reviews/:id/agent-turn { "message": string }` → `200 { reply, agent_session_id }`；無 session 或非 `draft` → `409`；agent 執行失敗 → `502`。
- manifest（`mode="mr_poll"`）新增欄位對齊 `schema.md §4.3`：`draft_dir`、`pending_dir`、`reviewer_username`、`since`（可選）、`eligible_mrs_path`（triage 輸出 JSON 路徑，通常與 manifest 同目錄的 `eligible_mrs.json`）、`mr_review_skip_labels`（JSON 陣列）、`mr_review_require_label`（可選字串）。
- triage 輸出 `eligible_mrs.json` 契約：`eligible[]` 每項含 `mr_iid`（int）、`mr_title`、`source_branch`、`author_identity`、`review_round`（1|2）；`skipped[]` 每項含 `mr_iid`、`skip_reason`。
- headless 草稿檔 frontmatter 契約：`mr_iid`（int，必填）、`mr_title`（string）、`review_round`（int，必填，1 或 2）、`author_identity`（string，MR author 的 email 或 glab username，供後端比對 `person_identities` 得出 `person_id`；比對不到則 `person_id=NULL`）。

**失敗模式**：
- triage script 失敗或 `eligible_mrs.json` 無法解析 → `run_projects.state='failed'`，不 spawn agent。
- triage 回傳 `eligible=[]` → `run_projects.state='done'`（成功 no-op），不 spawn agent。
- 掃描子行程逾時（沿用 `schedule_config.per_project_timeout_sec`）→ 該 MR 的 agent 子行程標記失敗，已成功產出的其他 MR 草稿仍解析入庫；若 orchestration 尚未拆分 per-MR timeout，則整個 `run_project` 標 `skipped_timeout`（實作時以 per-MR 子行程逾時為準，見 reviewer-execution spec）。
- 草稿檔 frontmatter 缺 `mr_iid` 或 `review_round`→ 該檔案跳過解析並記 warning log，不中斷其他草稿的解析。
- 子行程成功但 stdout 無 `session_id`→ 草稿仍入庫，`agent_session_id=NULL`；記 warning log。
- `agent-turn` 時 agent 子行程失敗（認證、逾時、session 過期）→ 回 `502`，不修改草稿內容與 `status`。
- `glab mr note` 失敗（網路、權限、MR 已關閉等）→ 發佈 API 回 `502`，`mr_reviews.status` 維持 `draft`，前端可重試。
- 去重判斷失效（GitLab notes 抓不到 `By: AI Agent` 標記，例如 note 被刪除）→ triage script 視為「無歷史記錄」，將 MR 列入 `eligible`（`review_round=1`）；不視為錯誤（寧可重複產草稿，不漏看）。

**驗收標準**：
- 整合測試：triage 回傳 `eligible=[]` 時，`run_project` 完成且不 spawn agent。
- 整合測試：triage 回傳 2 筆 eligible 時，僅 spawn 2 次 agent 子行程。
- 整合測試：對一個健康專案呼叫 `POST /api/projects/:id/mr-scan` 兩次，第二次在第一次未完成前應回 `409`。
- 整合測試：排程 cron 依 `mr_poll_interval_min=1`（測試用短週期）驗證確實每分鐘建立一個 `mr_poll` run。
- 整合測試：`mr_reviews` upsert 驗證同 `(project_id, mr_iid, review_round)` 重複解析草稿不會產生第二列，而是更新既有列的 `draft_md_path`/`agent_session_id`/`updated_at`。
- 整合測試：MR 掃描子行程 stdout 含 `session_id` 時，對應 `mr_reviews` 列的 `agent_session_id` 與 `reviewer_agent` 正確寫入。
- 整合測試：`POST /api/mr-reviews/:id/agent-turn` 對有 session 的 draft 回傳 agent 回覆；對 `agent_session_id=NULL` 回 `409`。
- 手動驗證：對測試 GitLab 專案跑一次完整流程（掃描→收件匣看到草稿→編輯→發佈→GitLab MR 上出現對應 note）。

**範圍邊界**：
- **In scope**：`scripts/triage-mrs.py`、`scan-mrs-headless` workflow、`mr_reviews` 後端模組與 API（含 `agent-turn`）、`runs.trigger` 擴充、MR 輪詢 cron、單專案手動觸發端點、去重鎖跨軌道共用驗證、`provision_mr_worktree`（on-demand MR worktree 建立）、per-MR 子行程 session 擷取與持久化。
- **Out of scope**：前端收件匣頁面與聊天 UI（`agent-turn` API 已預留供 v2 整合）、多使用者認證、per-manager 發佈身分、自動漏跑偵測、跨軌道觀察片段彙整進週報的實際解析程式碼（`_pending/` 落檔格式沿用 `spec.md §6.5` 描述，但軌道 1 消費 `_pending/` 的解析邏輯本身是既有 `reviewer-batch` workflow 的既定行為，非本次變更範圍）、MR worktree 的清理/GC 機制（合併或關閉後的 worktree 何時刪除，留待後續變更決定）、互動式 TUI 直接 `--resume` 開啟 agent（管理者可透過 API 或日後前端；本次僅 headless `agent-turn`）。

## Risks / Trade-offs

- [Risk] 高頻輪詢下若去重判斷（GitLab notes 標記）誤判為「已掃過」，會漏看真正的新 MR 或新一輪。 → Mitigation：去重邏輯偏向保守（判斷不到標記就重新產出草稿，寧可重複也不漏看，見 Implementation Contract 失敗模式）。
- [Risk] `mr_poll` cron 週期可能與 `schedule_config.per_project_timeout_sec`（預設 600 秒）或專案數量疊加，造成掃描永遠追不上輪詢間隔（例如 60 分鐘輪詢但單專案掃描要 15 分鐘、專案數多）。 → Mitigation：沿用既有 `max_concurrency` worker pool 限制與去重鎖，追不上時新的排入會被鎖擋下（不會無限堆積），但需在維運文件註明「輪詢間隔需大於『專案數 / max_concurrency × 單專案平均耗時』」。
- [Risk] 草稿發佈後若管理者又編輯內容，`published_body` 與草稿原文不一致，未來若要做「已發佈觀察片段回溯」可能對不上原始 AI 判斷。 → Mitigation：`mr_reviews.published_body` 就是設計來記錄「實際發佈內容」而非原始草稿，屬預期行為（schema.md 已有此欄位設計意圖）。
- [Risk] Agent session 可能因 provider 清理政策或長時間未使用而失效，`agent-turn` 回 `502`。 → Mitigation：API 錯誤訊息區分 session 過期；管理者可重新觸發 MR 掃描產生新草稿與新 session。
- [Risk] Per-MR 子行程比整批掃描更耗時／token。 → Mitigation：GitLab notes 去重仍跳過無新動靜的 MR，不會對每個 MR 每次都 spawn；worker pool `max_concurrency` 限制並發。

## Migration Plan

- 資料庫：新增 migration `002_mr_review_agent_session.sql`（`agent_session_id`、`reviewer_agent`）與 `003_mr_review_project_gates.sql`（`projects.mr_review_skip_labels`、`projects.mr_review_require_label`）。`schedule_config.mr_poll_interval_min` 已存在於 `001_initial.sql`。
- 部署：新增 `scripts/triage-mrs.py` 與 `skills/scan-mrs-headless/WORKFLOW.md` 隨 app repo 一併部署（`$APP_ROOT/`，triage 由後端以 `python $APP_ROOT/scripts/triage-mrs.py --manifest <path>` 執行）。
- 回滾：若需停用軌道 2，將 `schedule_config.enabled` 之外另加判斷（或直接不啟動第二個 cron job）即可停止新掃描；已存在的 `mr_reviews` 草稿不受影響，收件匣 API 仍可讀取歷史資料。
