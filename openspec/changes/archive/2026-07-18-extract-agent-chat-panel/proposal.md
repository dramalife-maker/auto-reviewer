## Why

週報頁與 MR 收件匣各有一套幾乎同構的 Agent Chat 面板；後續 UX／版面調整若兩邊分開改，容易漂移。先抽出共用 UI 殼，才能在不破壞既有行為的前提下統一調整版面。

## What Changes

- 新增共用 `AgentChatPanel`（presentational）：標題／收合、訊息氣泡、可選 composer（textarea + 送出 + Enter 送出、loading 提示）
- `ReportsPage` 與 `MrInboxPage` 改為使用該共用元件；頁面各自保留 hydrate、`handleAgentTurn`、顯示條件、draft 衝突／週報 reload 等邏輯
- 以 props 表達兩邊差異：`readOnly`、`inputDisabled`、空狀態文案、placeholder、標題後綴、外層高度／寬度 class
- 既有 `MrInboxPage` 測試行為必須繼續通過；失敗回滾差異（週報撤回樂觀訊息、MR 不撤回）維持現狀

## Capabilities

### New Capabilities

- `agent-chat-panel`: Shared presentational Agent Chat shell (header, transcript, optional composer) used by report reader and MR inbox; page-owned turn orchestration stays outside the shell

### Modified Capabilities

(none)

## Impact

- Affected specs: `agent-chat-panel` (new); existing `frontend-shell` / `mr-agent-chat` / `report-reader-agent-chat` behavior requirements remain in force and MUST NOT regress
- Affected code:
  - New: `frontend/src/components/AgentChatPanel.tsx`（路徑可依專案慣例微調，但必須為單一共用元件檔）
  - Modified: `frontend/src/pages/ReportsPage.tsx`, `frontend/src/pages/MrInboxPage.tsx`, `frontend/src/pages/MrInboxPage.test.tsx`（若匯出／selector 變動需同步）
  - Removed: 頁內 `ReportChatPanel`、`MrReviewChatPanel` 重複實作（邏輯搬入共用元件後刪除）
