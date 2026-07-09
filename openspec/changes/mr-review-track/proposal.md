## Why

現有系統只有軌道 1（週報批次）。工程師在 MR 上的即時行為（code review 討論、架構決策）要等到週報才被看見，而且完全沒有機制產出 MR review 草稿協助管理者審查。`docs/idea/spec.md` 與 `schema.md` 已設計軌道 2（MR review），且 `mr_reviews` 表與 `schedule_config.mr_poll_interval_min` 欄位已預先建在 `backend/migrations/001_initial.sql`，但沒有任何後端模組、API 或排程實際使用它們——軌道 2 尚未落地。

## What Changes

- 新增 `mr-review` 能力：
  - 新增 **MR triage script**（`scripts/triage-mrs.py`）：在觸發 MR 掃描時由後端先執行，透過 `glab` 列出 open MR、解析 notes 做分輪／去重判斷，並套用**防呆排除規則**（GitLab Draft MR、排除標籤、可選的必備標籤），輸出 `eligible_mrs.json`（哪些 MR 需要 review）。此步驟**不啟動 AI agent**，節省 token。
  - 新增 headless workflow `skills/scan-mrs-headless/WORKFLOW.md`，改造自互動式 `cto:scan-mrs`：移除互動確認與 `glab mr note`/`glab mr merge` 副作用；**不再負責** `glab mr list`／MR 篩選（由 triage script 預先完成），僅對 manifest 指定的單一 MR 做 code review 並產出草稿檔與觀察片段。
  - 新增後端模組解析草稿檔並 upsert 進 `mr_reviews`（欄位對齊 `schema.md §5.1` 並擴充 `agent_session_id`/`reviewer_agent`：`project_id`/`person_id`/`mr_iid`/`mr_title`/`review_round`/`draft_md_path`/`status`/`agent_session_id`/`reviewer_agent`）。
  - MR 掃描子行程**啟用 agent session 持久化**（與週報軌道相反，不使用 `--no-session-persistence`），並從 `stream-json` stdout 擷取 `session_id` 寫入對應草稿列，供管理者對草稿有疑慮時接回同一 session 追問。
  - 新增 API：`GET /api/mr-reviews`（收件匣清單，篩 `status='draft'`）、`PATCH /api/mr-reviews/:id`（編輯草稿內容）、`POST /api/mr-reviews/:id/publish`（後端代跑 `glab mr note`，寫回 `published_at`/`published_body`，`status='published'`）、`POST /api/mr-reviews/:id/ignore`（`status='ignored'`）、`POST /api/mr-reviews/:id/agent-turn`（以儲存的 `agent_session_id` 接續 headless 對話，回傳 agent 回覆）。
  - 觀察片段落檔至 `reports/<project>/<person>/_pending/`；**發佈後**才視為進入軌道 1 週報彙整素材（依 `spec.md §6.5`）。
  - 新增單一專案手動觸發 MR 掃描：`POST /api/projects/:id/mr-scan`，比照現有 `manual_project` 週報觸發模式，但走 `mode=mr_poll` manifest 與 `scan-mrs-headless` workflow。
- 修改 `scheduling` 能力：
  - 排程器新增第二個 cron job，依 `schedule_config.mr_poll_interval_min`（已存在的欄位，目前未被使用）定期對所有健康專案觸發 MR 掃描（`trigger='mr_poll'`）。
- 修改 `reviewer-execution` 能力：
  - `runs.trigger` 新增合法值 `mr_poll`（排程觸發）與 `manual_mr_poll`（單專案手動觸發），比照既有 `manual_all`/`manual_project` 的建立與去重鎖模式（同專案不可被軌道 1 與軌道 2 同時排入，共用同一把鎖）。
- 修改 `repo-worktree` 能力：
  - 新增 on-demand 建立 merge-request worktree 的邏輯：依 MR 來源分支 fetch 並建立/重用 worktree，套用既有 `escape_branch` 命名規則（含 hash 後綴避免碰撞，已在 spec 中定義但尚無呼叫端）。此邏輯供 `mr-review` 能力在每次掃描時取得 MR 專屬工作目錄。
- 修改 `project-config` 能力：
  - 專案新增 MR review 排除設定：`mr_review_skip_labels`（命中即跳過）、`mr_review_require_label`（可選，未帶此標籤則跳過）；triage script 透過 manifest 讀取。

## Non-Goals

- **不做自動漏跑偵測補跑**：無論軌道 1 或軌道 2，都不在此變更範圍內做「服務重啟時偵測上次應執行時間已過」的自動補跑機制。軌道 2 改以本次新增的單專案手動觸發取代（`spec.md §7.3` 原提案的漏跑偵測不採用）。
- **不做多使用者認證與 per-manager 發佈身分歸屬**：`mr_reviews.published_body` 由單一 service 帳號代跑 `glab mr note` 發佈，不記錄「哪位主管按下發佈」。`spec.md §8 #8` 與 memory `project-authz-roles` 提到的角色/認證設計延後到未來變更。
- **不做「全部專案一鍵掃 MR」**：手動觸發僅支援單一專案，比照週報既有的單專案入口；全域「立即掃描」按鈕不在本次範圍。
- **不建立獨立的第二套 clone/fetch 系統**：MR worktree 建立沿用既有 bare repo（`.bare/`）與 `provision_project` 已建立的 remote 設定，只新增「依分支名 fetch＋建立 worktree」這個動作，不重新設計 clone 流程。
- **不做前端 MR 收件匣 UI**：本次範圍為後端 API 與 headless workflow；`spec.md §10` 標示為 v2 的收件匣頁面留待後續變更。`agent-turn` API 供未來收件匣或 CLI 整合，本次不實作聊天 UI。專案設定的 MR 排除標籤欄位可先透過 `projects.yaml`／API 設定，專案設定頁 UI 可後續補上。

## Capabilities

### New Capabilities

- `mr-review`: MR 輪詢軌道核心能力——**triage script 預篩 MR**、headless 掃描 workflow、草稿解析入庫（含 agent session 綁定）、收件匣 API（列表/編輯/發佈/忽略/接續對話）、觀察片段落檔、單專案手動觸發端點。

### Modified Capabilities

- `scheduling`: 新增 MR 輪詢 cron job，依 `mr_poll_interval_min` 週期觸發所有健康專案的 MR 掃描。
- `reviewer-execution`: `runs.trigger` 新增 `mr_poll`／`manual_mr_poll`，去重鎖規則擴及與軌道 1 共用；MR 掃描子行程啟用 session 持久化並解析 `stream-json` 取得 `session_id`。
- `repo-worktree`: 新增 on-demand merge-request worktree 建立（fetch 指定分支＋套用既有命名規則），供 MR 掃描使用。
- `project-config`: 專案級 MR review 排除設定（skip labels、optional require label），供 triage script 讀取。

## Impact

- Affected specs: mr-review (new), scheduling (modified), reviewer-execution (modified), repo-worktree (modified), project-config (modified)
- Affected code:
  - New:
    - scripts/triage-mrs.py
    - skills/scan-mrs-headless/WORKFLOW.md
    - backend/src/mr_reviews.rs
  - Modified:
    - backend/src/schedule.rs
    - backend/src/runs.rs
    - backend/src/server.rs
    - backend/src/lib.rs
    - backend/src/worktree.rs
    - backend/src/executor.rs
    - backend/migrations/002_mr_review_agent_session.sql
    - backend/migrations/003_mr_review_project_gates.sql
    - backend/src/projects.rs
    - openspec/changes/mr-review-track/specs/project-config/spec.md
