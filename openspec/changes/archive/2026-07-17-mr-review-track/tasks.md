## 1. repo-worktree: on-demand MR worktree

- [x] 1.1 實作 `provision_mr_worktree(repo_path, source_branch) -> Result<PathBuf>`（`backend/src/worktree.rs`）：對 `.bare/` 執行 `git fetch origin <source_branch>`，再依既有 `escape_branch` 命名規則建立 worktree（不存在則建立，存在則重用並仍完成該次 fetch）。對齊設計決策「MR worktree 為 on-demand 建立，擴充既有 `repo-worktree` 能力而非新建系統」。驗證：新增單元測試，首次呼叫對不存在的分支建立 worktree 並回傳正確路徑（對應規格 Requirement: Merge-request worktrees are provisioned on demand during a scan 的 Scenario「First scan of a merge request creates its worktree」）。
- [x] 1.2 驗證兩個共用同一來源分支的 MR 解析到同一個 worktree 目錄、不重複建立。驗證：新增測試比對兩次 `provision_mr_worktree` 對同一 `source_branch` 回傳相同路徑，且檔案系統只存在一個對應目錄（對應 Scenario「Two merge requests sharing a source branch reuse one worktree」）。
- [x] 1.3 `fetch` 失敗時（分支已刪除或 remote 不可達）回傳錯誤但不中斷同專案其他 MR 的掃描。驗證：對不存在的分支呼叫 `provision_mr_worktree` 應回傳 `Err`，且呼叫端測試證明迴圈中其他分支仍被處理（對應 Scenario「Unreachable source branch skips only that merge request」）。

## 2. reviewer-execution: 觸發類型擴充與手動掃描端點

- [x] 2.1 `runs.trigger` 新增合法值 `mr_poll` 與 `manual_mr_poll`，並在 `backend/src/runs.rs` 新增 `create_manual_mr_scan_run(pool, project_id)` 依循既有 `create_manual_project_run` 模式（插入 `runs` 列 `trigger='manual_mr_poll'`、一列 `run_projects` `state='queued'`）。對齊設計決策「`runs.trigger` 擴充為四個合法值」。驗證：單元測試插入後查詢 `runs.trigger='manual_mr_poll'` 且對應 `run_projects` 存在。
- [x] 2.2 新增 `POST /api/projects/:id/mr-scan` handler（`backend/src/server.rs`），成功時回傳 `202 { run_id }`，若該專案已有 `queued`/`running` 的 `run_projects` 列（不分 trigger 來源）則回 `409`。驗證：整合測試對同一專案連續呼叫兩次，第二次得到 409（對應規格 Requirement: Manual MR scan enqueues a single project 的兩個 Scenario）。
- [x] 2.3 確認手動觸發與排程觸發共用同一去重鎖查詢路徑（不新增第二套鎖表）。對齊設計決策「去重鎖規則擴大到跨軌道」。驗證：整合測試對一個正在 `manual_project`（軌道 1）執行中的專案呼叫 `mr-scan`，應被 409 擋下。

## 3. scheduling: MR 輪詢 cron

- [x] 3.1 `start_scheduler`（`backend/src/schedule.rs`）新增第二個 `Job::new_async_tz`，依 `schedule_config.mr_poll_interval_min` 換算週期，對所有 `is_git_repo=1` 專案建立 `trigger='mr_poll'` 的 run，且獨立於週報 `schedule_config.enabled` 開關（兩軌節奏分開）。對齊設計決策「MR 輪詢 cron 為獨立第二個 job，而非合併進週報 cron」。驗證：新增測試以短週期（如 `mr_poll_interval_min=1` 分鐘）驅動排程器，斷言在等待時間內產生至少一筆 `trigger='mr_poll'` 的 `runs` 列（對應 Requirement: MR poll cron triggers scheduled scans on an independent interval 的第一個 Scenario）。
- [x] 3.2 驗證 MR 輪詢 cron 觸發時，已被軌道 1 鎖住的專案不會被重複排入，其餘健康專案仍正常排入。驗證：整合測試模擬一個專案處於 `trigger='schedule'` 的 `running` 狀態，觸發 MR 輪詢後該專案不出現在新 `run_projects` 中，其他專案出現（對應 Scenario「MR poll skips a project already locked by the weekly track」）。

## 3b. mr-review: triage script（MR 列舉與篩選，不經 AI）

- [x] 3b.1 建立 `scripts/triage-mrs.py`：讀取 manifest（`mode=mr_poll`），在 resident worktree 內以 `glab` 列出 open MR；**先套用防呆**（GitLab Draft、排除標籤、可選必備標籤），再依 `By: AI Agent` note 做分輪（1/2）與去重，輸出 `eligible_mrs.json`（`eligible[]` + `skipped[]`）。對齊設計決策「MR triage script 負責列舉與篩選」與「MR review 防呆」。驗證：以 fixture `glab` 輸出做單元測試，涵蓋 `gitlab_draft`、`label:wip`、`missing_required_label`、round-1、round-2-with-activity、skip-no-activity 等情境。
- [x] 3b.2 後端 MR 掃描 orchestration：在 spawn agent 前先執行 `python $APP_ROOT/scripts/triage-mrs.py --manifest <path>`；讀取 `eligible[]`，空陣列則 `run_project` 標 `done` 且不 spawn agent；script 失敗則標 `failed`。驗證：整合測試 mock triage 輸出，斷言 eligible 為空時零 agent spawn、eligible 有 2 筆時 spawn 2 次（對應 mr-review spec 三個 triage Scenario）。
- [x] 3b.3 manifest 寫入時新增 `eligible_mrs_path`、`mr_review_skip_labels`、`mr_review_require_label` 欄位（後兩者來自 `projects` 表）。驗證：單元測試斷言 MR poll manifest JSON 含這些欄位。

## 3c. project-config: MR review 防呆設定

- [x] 3c.1 新增 migration `003_mr_review_project_gates.sql`：`projects` 表加 `mr_review_skip_labels TEXT NOT NULL DEFAULT '["wip","do-not-review","no-ai-review"]'`、`mr_review_require_label TEXT`。驗證：migration 套用成功。
- [x] 3c.2 `projects.yaml` 載入與專案 API 支援讀寫上述兩欄位；未設定時使用預設 skip labels。對應 project-config spec。驗證：`project_config` 測試覆蓋 YAML 覆寫與 manifest 帶入。

## 4. mr-review: headless workflow

- [x] 4.1 建立 `skills/scan-mrs-headless/WORKFLOW.md`，改造自 `cto:scan-mrs` 的 review 本體邏輯（僅供邏輯參考）：移除互動確認與 `glab mr note`/`glab mr merge` 副作用；**不得**執行 `glab mr list` 做 MR 發現（已由 triage script 完成）。Workflow 以 manifest 的 `mr_iid` + `eligible_mrs.json` 內該 MR 的 metadata 為範圍，每次子行程只處理單一 MR；變更用後端預算的 `change_*` 檔（基準 `origin/<target_branch>...HEAD`；**禁止** `glab mr diff` 與全量重跑 git diff）；允許 `glab mr view` 取得討論脈絡；草稿優先於觀察、禁止廣掃 reports。驗證：`grep` 確認 WORKFLOW.md 不含 `glab mr list`、不含 `glab mr note`/`glab mr merge`/`glab mr diff`/互動式問句。
- [x] 4.2 WORKFLOW.md 定義草稿檔輸出格式（frontmatter 含 `mr_iid`/`mr_title`/`review_round`/`author_identity`）落地至 manifest 指定的 `draft_dir`，並將工程師觀察片段寫入 `reports/<project>/<person>/_pending/`。驗證：以固定測試用 manifest（含單一 `mr_iid`）手動執行一次 workflow，確認 `draft_dir` 下產出的檔案 frontmatter 符合契約欄位。
- [x] 4.3 後端在 spawn 每個 MR agent 前預寫 `change_log.txt` / `change_stat.txt` / `change.diff`（diff 有大小上限），並把路徑寫入 per-MR manifest。驗證：`mr_change_materials` 單元測試涵蓋產出與截斷；manifest 測試含三個 path 欄位。

## 4b. reviewer-execution: MR 子行程 session 持久化

- [x] 4b.1 新增 migration `002_mr_review_agent_session.sql`：為 `mr_reviews` 加上 `agent_session_id TEXT`、`reviewer_agent TEXT NOT NULL DEFAULT 'cursor'`。驗證：migration 在空庫與既有 `001` 庫上皆可套用。
- [x] 4b.2 在 `backend/src/executor.rs` 新增 MR 掃描專用執行路徑（例如 `execute_mr_scan`）：per-MR 子行程不傳 `--no-session-persistence`（Claude），沿用 `stream-json` stdout；週報路徑 `execute_weekly_batch` 行為不變。驗證：單元測試斷言兩條路徑的 command args 差異（對應 reviewer-execution spec Scenario「Weekly batch still disables…」與「MR scan subprocess omits…」）。
- [x] 4b.3 實作 `parse_agent_session_id(stdout, reviewer_agent) -> Option<String>`：從 NDJSON 擷取 Claude `session_id` 或 Cursor `system/init`/`result` 的 `session_id`。驗證：單元測試餵入 fixture stdout，涵蓋兩種 agent 與缺 session 的情況。

## 5. mr-review: 草稿解析與收件匣入庫

- [x] 5.1 新增 `backend/src/mr_reviews.rs`：掃描子行程完成後解析 `draft_dir` 下的草稿檔，依 `author_identity` 比對 `person_identities` 解析 `person_id`（比對不到則為 `NULL`），以 `(project_id, mr_iid, review_round)` upsert 進 `mr_reviews`，`status='draft'`，並寫入該 MR 子行程擷取到的 `agent_session_id`/`reviewer_agent`。對應規格 Requirement: MR draft is parsed… 與 MR scan subprocess persists agent session。驗證：單元測試——新草稿產生新列且 session 綁定正確、重複解析更新 `agent_session_id`（兩個 Scenario）。
- [x] 5.2 缺少 `mr_iid` 或 `review_round` 的草稿檔跳過解析並記錄 warning，不中斷其他檔案解析。驗證：單元測試餵入一份缺 `mr_iid` 的檔案與一份完整檔案，斷言只有完整檔案產生 `mr_reviews` 列（對應 Scenario「Draft missing required frontmatter is skipped」）。

## 6. mr-review: 收件匣 API

- [x] 6.1 新增 `GET /api/mr-reviews`（預設 `status=draft`，支援 query 篩選 `draft`/`published`/`ignored`），回傳列表含 `agent_session_id` 與 `reviewer_agent`，依 `created_at` 遞減排序。對應規格 Requirement: MR review inbox lists draft entries。驗證：整合測試建立 2 筆 draft、1 筆 published，呼叫無參數的 API 應只回傳 2 筆 draft 且含 session 欄位（對應 Scenario「Inbox returns only draft status by default」）。
- [x] 6.2 新增 `PATCH /api/mr-reviews/:id`，`status='draft'` 時覆寫 `draft_md_path` 檔案內容且不呼叫 GitLab；`status` 非 `draft` 時回 `409`。對應規格 Requirement: Draft content can be edited before publishing。驗證：整合測試覆寫一筆 draft 的內容後讀檔比對；另測試對 `published` 列呼叫 PATCH 得到 409（對應兩個 Scenario）。
- [x] 6.3 新增 `POST /api/mr-reviews/:id/publish`，呼叫 `glab mr note <mr_iid> --message <draft_body>`（`cwd`=該專案 resident worktree），成功寫回 `status='published'`/`published_at`/`published_body`；失敗回 `502` 且維持 `draft`。對應規格 Requirement: Publishing a draft posts to GitLab and records the published body。驗證：整合測試以測試替身（fake `glab`）驗證成功與失敗兩條路徑（對應兩個 Scenario）。
- [x] 6.4 新增 `POST /api/mr-reviews/:id/ignore`，只更新 `status='ignored'`，不呼叫任何 GitLab 指令。對應規格 Requirement: Ignoring a draft never contacts GitLab。驗證：整合測試呼叫後檢查 `status='ignored'` 且測試替身 `glab` 未被呼叫（對應 Scenario「Ignoring a draft changes status only」）。
- [x] 6.5 新增 `POST /api/mr-reviews/:id/agent-turn`，以 `agent_session_id` + `reviewer_agent` 接續 headless 對話，回傳 `{ reply, agent_session_id }`；無 session 或非 `draft` 回 `409`；agent 失敗回 `502`。對應規格 Requirement: Draft agent session can be continued for clarification。驗證：整合測試以 fake agent 驗證成功、409（無 session）、409（published）三條路徑。

## 7. mr-review: 觀察片段與週報打通

- [x] 7.1 `reviewer-batch` workflow（既有）在彙整週報時，對 `_pending/` 下每個觀察片段查詢對應 `mr_reviews.status`：僅 `status='published'` 才折入 `summary.md` 並從 `_pending/` 移除，`draft`/`ignored` 保留原地不動。對應規格 Requirement: Observation snippets are consumed by the weekly track only after publish。驗證：整合測試分別放入一個對應 `published` 列與一個對應 `draft` 列的片段，執行週報彙整後，前者被折入 `summary.md` 且從 `_pending/` 消失，後者仍留在 `_pending/`（對應兩個 Scenario）。

## 8. 整合驗證

- [x] 8.1 端到端手動驗證：對測試用 GitLab 專案執行一次完整流程——手動觸發掃描（`POST /api/projects/:id/mr-scan`）→ 收件匣看到草稿與 `agent_session_id`（`GET /api/mr-reviews`）→ 對草稿追問（`POST .../agent-turn`）→ 編輯內容（`PATCH`）→ 發佈（`POST .../publish`）→ 確認對應 GitLab MR 上出現該則 note。驗證：手動執行並記錄結果於 PR 描述或測試報告。
