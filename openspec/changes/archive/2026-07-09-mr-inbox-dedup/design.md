## Context

MR review 軌道（`mr-review-track`）已實作：`scripts/triage-mrs.py` 在 resident worktree 內列舉 open MR、套用標籤防呆、並依 GitLab notes 是否含 `By: AI Agent` 決定 `review_round` 與是否 eligible。Triage 輸出 `eligible_mrs.json` 後，Rust worker 對每筆 eligible 項目 provision worktree 並 spawn headless agent，結果 upsert 至 `mr_reviews`（狀態 `draft`）。

問題：未發佈的 `draft` 在 GitLab 上尚無 AI note，下一輪 triage 仍視為 round 1 eligible；worker 不查 SQLite，會再次 spawn agent 並覆寫草稿。`ignored` 列同理會被重掃。

既有設計刻意讓 triage 不碰 SQLite（保持腳本可單獨測試、無 DB 依賴）。收件匣閘門應放在 Rust worker，在 triage 與 agent spawn 之間。

## Goals / Non-Goals

**Goals:**

- 排程與一般手動 MR 掃描在 spawn agent 前跳過收件匣中 `draft` / `ignored` 的 `(project_id, mr_iid, review_round)`。
- 記錄被跳過項目的原因（`inbox_draft`、`inbox_ignored`），便於日誌與除錯。
- 手動 `force=1` 可繞過閘門，讓使用者在新 commit 後強制重掃未發佈草稿。
- `publish` 發佈至 GitLab 時附加 `By: AI Agent` footer，與 triage 去重標記對齊。

**Non-Goals:**

- 不修改 `scripts/triage-mrs.py` 讀取 SQLite。
- 不實作「有新 commit 自動重掃未發佈草稿」— 仍須手動 `force=1`。
- 不變更 `published` 列行為；已發佈輪次由 triage GitLab note 邏輯處理。
- 不阻擋 `agent-turn` 或收件匣 UI 編輯流程。

## Decisions

### Worker 收件匣閘門位置（triage 之後、spawn 之前）

在 `process_mr_run_project` 讀取 `eligible_mrs.json` 之後、進入 per-MR 迴圈之前呼叫過濾函式。Triage 腳本與 manifest 流程不變。

**替代方案：** 在 triage 腳本查 SQLite — 拒絕，違反腳本無 DB 依賴、且需重複連線設定。

### 比對鍵與擋住狀態

查詢 `mr_reviews` 中 `status IN ('draft', 'ignored')` 的 `(project_id, mr_iid, review_round)` 集合。Eligible 項目中若三元組命中集合則跳過；`published` 不擋（由 triage 處理）。

**替代方案：** 僅擋 `draft` — 拒絕，`ignored` 也應避免重複消耗 agent。

### force 繞過閘門

`POST /api/projects/:id/mr-scan` 接受 query `force=1`（或 `force=true`）。建立 run 時將 `force` 存入 run 或 run_projects metadata（例如 `runs` 表現有欄位或 manifest 旁 JSON 旗標），worker 讀取後跳過收件匣過濾。排程 `mr_poll` trigger 永遠不帶 force。

**替代方案：** 獨立 API `POST .../mr-scan/force` — 拒絕，query 參數較少表面積。

### publish 附加去重標記

`mr_reviews::publish` 在呼叫 `glab mr note` 前，將 `draft_body` 與固定 footer（與 triage `AI_AGENT_MARKER` 一致：`By: AI Agent`）合併；若正文已含該標記則不重複附加。`published_body` 存實際張貼內容（含 footer）。

## Implementation Contract

**Behavior**

- 排程 MR 掃描：若 triage 列出 MR 5 round 1，且 DB 已有 `project_id=P, mr_iid=5, review_round=1, status=draft`，worker 不 spawn agent，日誌含 skip 原因；run 仍可 `done`（無 eligible 可執行時與現有「空 eligible」行為一致）。
- 手動掃描 `?force=1`：同上情境仍 spawn agent 並 upsert 覆寫草稿。
- 發佈：GitLab note 末尾含 `By: AI Agent`；DB `published_body` 與張貼內容一致。

**Interface / data shape**

- `load_inbox_blocked_rounds(pool, project_id) -> HashSet<(mr_iid, review_round)>`（或等價結構）
- `filter_eligible_by_inbox(eligible, blocked, force: bool) -> (to_run, skipped)`，`skipped` 含 `mr_iid`, `review_round`, `skip_reason`
- `POST /api/projects/:id/mr-scan?force=1` — 語意不變（仍 409 衝突、202 接受），僅多繞過閘門
- Publish：note body = `draft_body` + 必要時 `\n\nBy: AI Agent`

**Failure modes**

- DB 查詢失敗：整個 MR run project 標 `failed`（與 triage 失敗一致），不部分 spawn。
- force 參數缺失或 `0`：正常閘門行為。

**Acceptance criteria**

- `cargo test` 閘門單元測試：draft/ignored 過濾、force 不過濾、published 不擋。
- 整合測試：mock eligible + DB seed，斷言 spawn 次數。
- 手動：`mr-scan` 無 force 跳過、`force=1` 重跑；publish 後 GitLab note 含標記。

**Scope boundaries**

- In scope: `mr_reviews.rs` 閘門函式、worker 串接、server force 參數、publish footer、前端強制重掃、測試。
- Out of scope: triage 腳本、排程間隔 UI、收件匣列表 API 變更。

## Risks / Trade-offs

- [未發佈草稿有新 commit 仍被擋] → 使用者用手動 force 重掃；文件與 UI 標示清楚。
- [publish footer 與 triage 標記字串不一致] → 共用常數或註解對齊 `scripts/triage-mrs.py` 的 `AI_AGENT_MARKER`。
- [force 旗標未傳到 worker] → 整合測試覆蓋 manual_mr_poll + force。

## Migration Plan

無 schema migration。部署後行為立即生效：既有 `draft`/`ignored` 列開始擋排程重掃。使用者若需重掃，使用 force。

## Open Questions

（無）
