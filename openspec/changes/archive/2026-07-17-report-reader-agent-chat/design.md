## Context

MR 收件匣的 Agent Chat（`POST /api/mr-reviews/:id/agent-turn`）依賴 scan 留下的 `agent_session_id` 做 `--resume`，並把 turn 寫入 `mr_review_chat_messages`。週報 batch 對 Claude 使用 `--no-session-persistence`，沒有可接續的週報 session。報告閱讀器目前只讀 `GET .../reports/latest`，無法與 Agent 討論或改檔。

管理者選中某人後，需要在閱讀器內對話，決定並直接調整該人目前可見的報告產物（週報 `summary.md`、`_pending` 觀察片段、人物層長期觀察等）。Agent 改完 `summary.md` 後，SQLite 的 `reports`／`pending_items` 也必須同步，避免 UI 與 DB 脫節。

## Goals / Non-Goals

**Goals:**

- 每人一個可持久化的報告 Agent Chat（session + 訊息）
- 首次發言可開新 session；之後 `--resume`
- Agent 可在鎖定路徑內讀寫報告檔；回覆後 UI reload latest reports
- **改檔後 ingest**：agent-turn 成功後，對該人各專案下的 `summary.md` 重跑與週報相同的 DB 同步（`reports`／`pending_items`／`## 已釐清` resolve）
- Chat UX 對齊 MR 收件匣（可摺疊面板、reload歷史）

**Non-Goals:**

- 不接續週報 batch session
- 不修改 MR 草稿、不呼叫 `glab`、不發佈 GitLab note
- 不做多管理者／認證隔離（與現況單機一致）
- **不**重跑完整週報 batch／MR scan（只做該人 summary 的 DB ingest）
- 不把 chat 計入 sidebar badge

## Decisions

### 儲存：`person_report_chats` + `person_report_chat_messages`

- `person_report_chats`：`person_id` UNIQUE、`agent_session_id`（可 NULL 直到首次成功 turn）、`reviewer_agent`、`updated_at`
- `person_report_chat_messages`：`person_id` FK CASCADE、`role`（user|assistant）、`content`、`created_at`；以 `id` ASC 為序
- 否決塞進 `people` 表 JSON：與 `persist-mr-agent-chat` 風格不一致

### Session：每人一條；首次 turn 開新 session

- 無 `agent_session_id` 時：executor **不帶** `--resume`，開新對話並解析 stdout 寫回 session
- 有 session 時：`--resume`（與 MR agent-turn 相同 agent／model 設定）
- 週報 batch 的 no-session-persistence 不變

### Agent 工作目錄與可寫範圍

- `--add-dir` 含 `DATA_ROOT`（與 MR turn 類似），prompt／system 明確限制只可改：
  - `reports/<project>/<display_name>/`（含 `summary.md`、日期目錄、`_pending/`、月檔）
  - `reports/_people/<display_name>/`（`index.md`、月趨勢、`_notes.md`）
- `cwd`：優先用 `DATA_ROOT`（跨專案報告）；不必綁單一 git worktree
- 禁止寫入：`runs/`、MR `draft_dir`、他人員資料夾、`.notes` ADR（除非路徑恰落在上述允許樹——預設禁止專案 `.notes`）

### API

- `GET /api/people/:id/report-chat` → `{ agent_session_id, reviewer_agent, chat_messages[] }`（人物不存在 404；尚無 chat 列則空訊息 + null session）
- `POST /api/people/:id/report-chat/agent-turn` body `{ "message": string }` → `{ reply, agent_session_id, ingest_warnings? }`；成功後寫入兩則訊息；agent 失敗 502 且不寫訊息
- 成功 turn **之後**執行「改檔後 ingest」（見下）；前端再 `fetchLatestReports`，DB 與磁碟一致

### 改檔後 ingest（person-scoped summary reingest）

- Agent 子行程成功且 chat 已持久化後，後端 MUST 掃描該人 `display_name` 在各 `reports/<project>/<display_name>/` 下的 `summary.md`，走與 `ingest_project_summaries`／`upsert_summary` **相同**的 DB 契約（更新 `one_line`／counts、open `pending_items`、`## 已釐清` resolve）
- `run_id`：若 `(project_id, person_id, report_date)` 已有列 → **保留原 `run_id`**；若需新建列 → 使用該人該專案最近一筆 report 的 `run_id`，若仍無則 warn 並略過該檔（chat 編輯預期改既有週報）
- Ingest 錯誤 **不得**把已成功的 agent-turn 改成 502；log warn，回應可含 `ingest_warnings: string[]`（無則省略或空陣列）
- `_pending` 觀察片段、人物層 `index.md` **不經**此 ingest（仍由 latest／trends 直接讀檔）

### UI：ReportsPage 右側／底部 Chat 面板

- 選人後顯示；切人重置 hydrate
- 送出 turn 成功後：`fetchLatestReports` 再取一次
- 不提供「套用／發佈」按鈕（檔案已被 agent 直接改）

## Implementation Contract

**Behavior**

- 選人開啟報告閱讀器可見 Agent Chat；重整後仍見歷史
- 管理者可要求調整某專案週報或 pending 觀察；agent 改檔後回覆說明；後端 ingest 同步 DB；UI reload 顯示更新後內容

**Interface**

- 上述 GET／POST JSON 形狀（POST 可含可選 `ingest_warnings`）
- Migration 表名與欄位如上
- Person-scoped summary reingest 函式（名稱實作自訂）可被 agent-turn 呼叫

**Failure modes**

- 未知 person → 404
- 空 message → 400
- agent 子行程失敗 → 502、不新增 chat 列、不做 ingest、保留既有 session_id（若有）
- 缺 session 的首次 turn 若 stdout 解析不到 session → 仍可回傳 reply，但 `agent_session_id` 維持 null 並 warn（下一次仍走「開新 session」路徑）
- ingest 部分失敗 → 仍 200；warnings／logs；chat 已寫入

**Acceptance**

- 整合測試：首次 turn 建立 session + 兩則訊息；第二次 resume；失敗不寫訊息
- 整合測試：agent-turn 改寫某 `summary.md` 的 `one_line`／待確認後，DB `reports`／`pending_items` 與之相符
- 前端：有歷史可 hydrate；turn 後 reports reload

**Scope**

- In：person report chat API + ReportsPage 面板 + executor 新開／resume + 改檔後 person summary ingest
- Out：MR inbox、完整週報 batch／MR scan、GitLab、認證

## Risks / Trade-offs

- [Risk] Agent 越權改他檔 → Mitigation：prompt 硬性路徑清單 + 僅 add-dir data_root；後續可加 tool wrapper
- [Risk] ingest 掃到未改的舊 summary 也重寫 DB → Mitigation：可接受（幂等 upsert）；只掃該人目錄
- [Risk] 與 MR chat 兩套相似程式 → Mitigation：共用 executor 層；API／表分離避免耦合
