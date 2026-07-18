## Why

週報與 MR 收件匣的 Agent Chat 目前以右側常駐欄呈現，即使收合仍佔窄欄並擠壓主內容。改為 floating button + overlay，讓主內容預設全寬，需要時再開聊天。

## What Changes

- **BREAKING（UI）**：移除右側常駐／窄條收合欄；關閉時改為固定右下角 FAB，開啟時以 overlay 面板浮在內容上
- 週報頁與 MR 收件匣共用同一套 floating 殼（建議 `AgentChatLauncher` 或等效），內嵌既有 `AgentChatPanel`
- 預設關閉（只見 FAB）；開啟後可關閉回到 FAB
- MR：無可顯示 chat 時（非 draft 且無歷史）不渲染 FAB，維持既有可見性規則
- 頁面仍擁有 hydrate、`handleAgentTurn`、唯讀／session 門檻；不改後端 API

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `agent-chat-panel`: 從側欄 chrome 改為 FAB + overlay；預設關閉；保留 presentational 邊界與頁面專屬 turn／可見性行為

## Impact

- Affected specs: `agent-chat-panel`
- Affected code:
  - New: `frontend/src/components/AgentChatLauncher.tsx`（名稱可微調，但須為兩邊共用的 open/close chrome）
  - Modified: `frontend/src/components/AgentChatPanel.tsx`（若收合按鈕語意改為關閉 overlay）、`frontend/src/pages/ReportsPage.tsx`、`frontend/src/pages/MrInboxPage.tsx`、`frontend/src/components/AgentChatPanel.test.tsx`、`frontend/src/pages/ReportsPage.agentChat.test.tsx`、`frontend/src/pages/MrInboxPage.test.tsx`
  - Removed: 頁面內右側 `Card` 側欄／`w-12` 窄條 Chat 入口

