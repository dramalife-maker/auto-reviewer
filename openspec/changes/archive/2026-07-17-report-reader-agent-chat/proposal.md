## Why

管理者在報告閱讀器看某人的週報／待折入觀察時，需要與 Agent 討論「該改哪一份報告」並直接改檔；目前只有 MR 收件匣有 Agent Chat，且依賴 MR scan 的 `--resume` session，週報 batch 又使用 `--no-session-persistence`，無法接續。

## What Changes

- 報告閱讀器（選人後）新增 Agent Chat 面板，可延續／開啟該人專用的 agent session
- 新增 API：讀取聊天紀錄、送出 turn；成功 turn 持久化；允許 agent 在約定路徑內 Read／Write 該人的報告產物
- turn 成功後對該人 `summary.md` 做 DB ingest（同步 `reports`／`pending_items`），前端再重新載入 latest reports
- Agent 寫入範圍鎖在該人報告目錄（各專案 `reports/<project>/<person>/` 與人物層 `reports/_people/<person>/`），不可動 MR 草稿或呼叫 GitLab

## Capabilities

### New Capabilities

- `report-reader-agent-chat`: Person-scoped Agent Chat for discussing and editing that person's report artifacts

### Modified Capabilities

- `report-reader`: Report reader UI hosts the Agent Chat panel for the selected person

## Impact

- Affected specs: `report-reader-agent-chat` (new), `report-reader` (modified)
- Affected code:
  - New: `backend/migrations/`（person report chat session／messages）, `backend/src/`（report chat module）
  - Modified: `backend/src/server.rs`, `backend/src/executor.rs`, `backend/src/summary.rs`, `frontend/src/pages/ReportsPage.tsx`, `frontend/src/api.ts`, `frontend/src/types.ts`
