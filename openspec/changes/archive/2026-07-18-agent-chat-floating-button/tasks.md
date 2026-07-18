## 1. 共用 floating 殼

- [x] 1.1 依 Decision: Shared launcher wraps the panel 與 Decision: Overlay does not push layout，新增 `AgentChatLauncher`：關閉時右下 FAB、開啟時 overlay 內嵌 `AgentChatPanel`、關閉控制回到 FAB；launcher 不擁有 turn 邏輯（Requirement: Shared Agent Chat panel is presentational）。驗證：launcher 元件測試涵蓋預設關閉見 FAB、點開 overlay、關閉回 FAB（Requirement: Agent Chat opens from a floating button into an overlay；Requirement: Agent Chat overlay defaults to closed）
- [x] 1.2 依 Decision: Close control replaces side-rail collapse，調整 `AgentChatPanel` 關閉控制語意／aria 對齊關閉 overlay（不再暗示側欄收合）。驗證：`AgentChatPanel` 測試更新後通過

## 2. 接线頁面

- [x] 2.1 [P] `ReportsPage` 改掛 launcher、移除側欄／窄條；預設關閉；保留 hydrate 與 `handleAgentTurn`。驗證：Reports Agent Chat 測試改為經 FAB 開啟，失敗回滾／成功 reload 仍成立（Decision: Default closed；Requirement: Page-specific Agent Chat behavior is preserved after extraction）
- [x] 2.2 [P] `MrInboxPage` 改掛 launcher；`showChatPanel` 為 false 時不渲染 FAB；唯讀／session 門檻不變。驗證：`MrInboxPage.test.tsx` Agent Chat 案例更新後全綠，含無歷史無 FAB（Decision: Keep page visibility gates；Requirement: Page-specific Agent Chat behavior is preserved after extraction）

## 3. 回歸

- [x] 3.1 跑相關前端測試並確認無側欄 chat chrome 殘留。驗證：測試綠燈；搜尋無 `w-12` Chat 窄條與並排 chat 側欄入口

