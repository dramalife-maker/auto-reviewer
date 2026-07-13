## Why

MR 收件匣的 Agent Chat 目前只存在前端記憶體；重新整理後追問脈絡會消失。同時 agent 在追問時可能直接改本地草稿檔，若使用者正在編輯會與磁碟新版靜默衝突，需要可察覺的「有新版本」與衝突處理。

## What Changes

- 新增 SQLite 表持久化每則 Agent Chat 訊息（綁定 `mr_reviews`）
- `POST /api/mr-reviews/:id/agent-turn` 成功後寫入 user + assistant 兩則訊息，並重讀 `draft_md_path` 回傳最新 `draft_body`
- `GET /api/mr-reviews` 每筆帶上 `chat_messages`，前端重整後還原對話
- 已發布／已忽略列唯讀顯示歷史；僅 draft 且有 session 可繼續追問
- Agent 改草稿視為預期行為；收件匣草稿區在內容變動時標記「有新版本」，dirty 衝突時提供預覽新版本／載入新版／保留編輯（不做行內 merge）
- `PATCH /api/mr-reviews/:id` 支援樂觀鎖定：帶上載入時的基準 hash，磁碟已變則 409 並回傳最新草稿

## Capabilities

### New Capabilities

- `mr-agent-chat`: Persist Agent Chat transcripts; re-read draft after agent-turn; optimistic draft save

### Modified Capabilities

- `frontend-shell`: Hydrate chat from API; read-only history for published/ignored; draft new-version badge, conflict preview, and edit choices

## Impact

- Affected specs: `mr-agent-chat` (new), `frontend-shell` (modified)
- Affected code:
  - New: `backend/migrations/013_mr_review_chat_messages.sql`
  - Modified: `backend/src/mr_reviews.rs`, `backend/src/server.rs`, `backend/tests/mr_reviews.rs`, `frontend/src/types.ts`, `frontend/src/api.ts`, `frontend/src/pages/MrInboxPage.tsx`, `frontend/src/pages/MrInboxPage.test.tsx`
