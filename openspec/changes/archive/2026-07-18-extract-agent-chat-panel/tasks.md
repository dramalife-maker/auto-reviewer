## 1. 共用殼

- [x] 1.1 依 Decision: Presentational shell only 與 Decision: Differences expressed as props，新增 shared `AgentChatPanel`：渲染標題（含可選後綴）、收合、氣泡列表、loading「AI 回覆中...」、非 readOnly 時的 composer（Enter 送出、`inputDisabled` 禁用）。驗證：新增元件測試涵蓋 readOnly 隱藏送出、`inputDisabled` 禁用、空狀態文案與 onSend（Requirement: Shared Agent Chat panel is presentational）

## 2. 接线頁面

- [x] 2.1 [P] `ReportsPage` 改用 `AgentChatPanel`，刪除頁內 `ReportChatPanel`；保留 hydrate、`handleAgentTurn`（失敗撤回樂觀訊息並還原 input、成功 reload 週報）。驗證：手動或針對週報 chat 的測試確認失敗回滾與成功 reload 仍成立（Requirement: Page-specific Agent Chat behavior is preserved after extraction；Decision: Preserve asymmetric failure rollback）
- [x] 2.2 [P] `MrInboxPage` 改用 `AgentChatPanel`，刪除頁內 `MrReviewChatPanel`；保留 `showChatPanel`、`agent_session_id` 門檻、唯讀、失敗不撤回、`handleIncomingDraft`。驗證：`MrInboxPage.test.tsx` 中 Agent Chat 相關案例全部通過（Decision: Keep page-level visibility gates；Requirement: Page-specific Agent Chat behavior is preserved after extraction）

## 3. 回歸確認

- [x] 3.1 跑前端相關測試（至少 `MrInboxPage` 與 `AgentChatPanel`），確認無重複面板實作殘留。驗證：測試綠燈；搜尋確認頁內不再定義 `ReportChatPanel` / `MrReviewChatPanel`

