## Context

Agent Chat 已抽出 presentational `AgentChatPanel`，但週報與 MR 收件匣仍以右側 `Card` 側欄承載（開 360/400px、關 `w-12`），預設開啟並擠壓主內容。討論結論：改為右下角 FAB + overlay，預設關閉；頁面邏輯不變。

## Goals / Non-Goals

**Goals:**

- 兩邊改為同一套 floating chrome（FAB 關閉、overlay 開啟）
- 主內容在 chat 關閉時佔滿可用寬度（不再留側欄／窄條）
- 預設關閉；MR 無可顯示 chat 時不渲染 FAB
- 保留 `AgentChatPanel` 與頁面 turn／可見性契約

**Non-Goals:**

- 不改後端 API、streaming、失敗回滾對齊
- 不做拖曳定位、多視窗、跨頁全域 chat
- 不引入第三方 chat widget
- 不改 `frontend-shell` 裡與 hydrate／唯讀無關的其他需求（僅透過 `agent-chat-panel` 表達 chrome 變更）

## Decisions

### Decision: Shared launcher wraps the panel

新增共用 `AgentChatLauncher`（名稱可等效）：負責 `open` 狀態、FAB、overlay 容器；children／props 轉傳給既有 `AgentChatPanel`。頁面仍傳 messages／input／onSend 等。

Alternatives considered:

- 只在各頁複製 FAB markup → 兩邊又漂移 → 否決
- 把 FAB 塞進 `AgentChatPanel` → 面板同時承擔 chrome 與 transcript，邊界變糊 → 否決

### Decision: Overlay does not push layout

開啟時 chat 以 `fixed`（或等效）浮層疊在內容上；關閉時不佔 flex 欄位。主內容列不再並排 chat `Card`。

### Decision: Default closed

`chatOpen`（或 launcher 內部／受控 `open`）預設 `false`。操作者點 FAB 才開 overlay。

### Decision: Keep page visibility gates

`MrInboxPage` 的 `showChatPanel` 仍決定是否掛載 launcher。false 時 FAB 與 overlay 皆不存在。週報在已選人時掛載 launcher。

### Decision: Close control replaces side-rail collapse

`AgentChatPanel` 的收合按鈕改為關閉 overlay（語意／aria 對齊「關閉」）；不再出現側欄窄條「Chat」文字入口。

## Implementation Contract

**Behavior**

- 關閉：右下角 FAB（可辨識為展開 Agent Chat）；主內容全寬
- 開啟：overlay 顯示 `AgentChatPanel`；關閉控制回到 FAB
- 預設關閉
- MR published／ignored 無歷史：無 FAB
- MR published／ignored 有歷史：有 FAB；開啟後唯讀 transcript、無送出
- Turn 成功／失敗行為與現況相同（含週報回滾、MR 不回滾、draft 衝突）

**Interface / data shape**

- Launcher 至少支援：受控或非受控 `open`、`onOpenChange`（或等效）、`visible`／由父層條件渲染、以及轉傳給 `AgentChatPanel` 的既有 props
- FAB：`aria-label` 含展開 Agent Chat 語意

**Failure modes**

- Launcher 不處理 API 錯誤；仍由頁面 `handleAgentTurn` 負責

**Acceptance criteria**

- 元件／頁面測試覆蓋：預設不見面板見 FAB；點 FAB 開 overlay；關閉回 FAB；MR 無歷史無 FAB；既有 Agent Chat 行為測試更新後仍綠
- 搜尋確認頁面不再渲染側欄 `w-12` Chat 窄條或並排 chat `Card` 欄位

**Scope boundaries**

- In scope: launcher、兩頁接线、`AgentChatPanel` 關閉語意、測試更新
- Out of scope: 動畫精修、持久化 open 狀態到 localStorage、鍵盤全域快捷鍵（除非實作極小且不影響契約）

## Risks / Trade-offs

- [Risk] Overlay 遮住編輯器重要操作 → Mitigation: 固定右下、可控寬高；關閉一鍵回到全寬
- [Risk] 測試仍假設側欄永遠可見 → Mitigation: 更新 `MrInboxPage`／Reports 測試改走 FAB 開啟
- [Trade-off] 預設關閉增加一次點擊 → 換取閱讀／編輯空間（討論已接受）

