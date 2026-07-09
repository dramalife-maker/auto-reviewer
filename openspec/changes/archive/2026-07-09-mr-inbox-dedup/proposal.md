## Why

排程 `mr_poll` 在 triage 通過後仍會對收件匣中尚未發佈的 `draft` 或已標記 `ignored` 的 MR 重新 spawn agent，覆寫同一 `(project_id, mr_iid, review_round)` 草稿並浪費 token。收件匣狀態存在 SQLite，但現有 triage 僅依 GitLab note 的 `By: AI Agent` 去重，兩套邏輯未對齊。

## What Changes

- 在 Rust worker 於 triage 完成後、spawn per-MR agent 之前，依 `mr_reviews` 收件匣狀態過濾 `eligible_mrs.json` 項目；`draft` 與 `ignored` 列不啟動 agent，並記錄 `skip_reason`（`inbox_draft` / `inbox_ignored`）。
- 比對鍵為 `(project_id, mr_iid, review_round)`，與現有 upsert 語意一致。
- `POST /api/projects/:id/mr-scan?force=1` 繞過收件匣閘門，供使用者在新 commit 出現時手動強制重掃。
- `publish` 成功發佈至 GitLab 時，在 note 末尾附加 `By: AI Agent` 標記，使 triage 下一輪能正確辨識已發佈輪次。
- 前端專案設定或 MR 掃描入口提供「強制重掃」選項（呼叫 `force=1`）。
- 單元測試與整合測試覆蓋閘門過濾、force 繞過與 publish 標記。

## Capabilities

### New Capabilities

- `mr-inbox-gate`: MR 掃描 worker 在 triage 之後依收件匣狀態過濾 eligible MR、手動 force 繞過、以及發佈時寫入 GitLab 去重標記。

### Modified Capabilities

（無）

## Impact

- Affected specs: `mr-inbox-gate`（新增）
- Affected code:
  - Modified: `backend/src/mr_reviews.rs`、`backend/src/worker.rs`、`backend/src/server.rs`、`backend/src/runs.rs`（若需傳遞 force 旗標）、`frontend/src/app.ts`、`frontend/src/api.ts`
  - New: `backend` 內閘門相關單元測試、整合測試 fixture
  - Removed: （無）
