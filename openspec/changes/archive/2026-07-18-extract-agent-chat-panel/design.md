## Context

週報頁（`ReportsPage`）與 MR 收件匣（`MrInboxPage`）各自實作右側 Agent Chat 面板，視覺與互動高度相似，但頁面邏輯不同：週報負責 person-report chat API、ingest warning 與週報 reload；MR 負責 draft session 門檻、唯讀歷史、以及 agent 回傳 draft 的新版本／衝突處理。後續 UX／版面調整需要單一 UI 殼，避免兩邊漂移。

## Goals / Non-Goals

**Goals:**

- 抽出單一 presentational `AgentChatPanel`，兩邊共用標題／收合、訊息列表、可選 composer
- 以 props 保留現有行為差異（唯讀、input disabled、文案、尺寸由外層控制）
- 頁面層 `handleAgentTurn`、hydrate、顯示條件維持在原頁，行為不因抽取而改變
- 既有 `MrInboxPage` Agent Chat 相關測試繼續通過

**Non-Goals:**

- 不改後端 API、streaming、中止 turn
- 不統一兩邊失敗回滾策略（週報撤回樂觀訊息；MR 失敗不撤回）
- 不做 markdown 渲染或其他 UX 視覺大改（本 change 只做抽取；後續版面調整另開 change）
- 不抽共用 hook／store；狀態仍由各頁 `useState` 管理

## Decisions

### Decision: Presentational shell only

共用元件只負責渲染與基本輸入事件（Enter 送出、onChange、onCollapse、onSend）。不呼叫 API、不擁有 chat messages 的來源真相。

Alternatives considered:

- 抽 `useAgentChat` hook：兩邊成功／失敗副作用差異大，過早抽象會把衝突邏輯混進共用層 → 否決
- 單一「智慧」Chat 元件吃 API endpoint：違反目前兩套後端契約與 MR draft 衝突流程 → 否決

### Decision: Differences expressed as props

至少支援：`messages`、`input`、`loading`、`readOnly`、`inputDisabled`、`emptyHint`、`placeholder`、`titleSuffix`（或等效 title）、`onInputChange`、`onSend`、`onCollapse`、可選 `className`。面板外層 `Card` 寬度與高度仍由各頁決定（週報約 360px + `min(70vh,720px)`；MR 約 400px + `h-full`）。

### Decision: Keep page-level visibility gates

`MrInboxPage` 的 `showChatPanel`（draft 或有歷史才顯示欄位）與 published／ignored 唯讀規則留在頁面；共用殼在 `readOnly` 時隱藏 composer。週報頁維持有選人即顯示 chat 欄。

### Decision: Preserve asymmetric failure rollback

抽取時不得「順便修好」MR 失敗不撤回樂觀 user 氣泡的行為；與週報撤回策略的對齊若需要，另開 change。

## Implementation Contract

**Behavior**

- 週報與 MR 收件匣的 Agent Chat 對外行為與抽取前相同（hydrate、送出、唯讀、隱藏條件、draft 衝突／週報 reload）
- 共用殼：標題「Agent Chat」+ 可選後綴；收合按鈕；氣泡列表；loading 文案「AI 回覆中...」；非唯讀時顯示 textarea +「送出」；Enter（不含 Shift）觸發 `onSend`

**Interface / data shape**

- 訊息項：`{ role: 'user' | 'assistant'; text: string }`
- 殼為 controlled：`input` / `onInputChange`；送出由父層 `onSend` 執行
- `readOnly === true` → 不渲染 composer
- `inputDisabled === true` → textarea／送出依現有 MR 規則（例如缺少 `agent_session_id`）禁用

**Failure modes**

- 殼本身不處理 API 錯誤；錯誤 toast／樂觀更新回滾由父層既有 `handleAgentTurn` 負責
- MR：失敗時樂觀 user 訊息保留、input 已清空（現狀）
- 週報：失敗時移除最後一則樂觀 user 訊息並還原 input（現狀）

**Acceptance criteria**

- `frontend` 既有 `MrInboxPage` 測試中與 Agent Chat 相關案例全部通過（hydrate、published 唯讀、無歷史隱藏、agent draft 新版本／衝突）
- 週報頁手動或既有測試可確認：送出成功會 append assistant；失敗撤回 user 氣泡並還原 input
- 程式庫中不再保留頁內重複的 `ReportChatPanel` / `MrReviewChatPanel` 實作本體

**Scope boundaries**

- In scope: 共用 UI 殼抽取與兩邊接线、必要時補最小單元測試
- Out of scope: UX 視覺重設計、streaming、失敗回滾對齊、後端變更

## Risks / Trade-offs

- [Risk] 抽取時誤把 `handleIncomingDraft` 或失敗回滾「統一」→ Mitigation: 任務明確禁止改 turn 副作用；以既有 MR 測試當回歸閘
- [Risk] props 過度泛化導致難用 → Mitigation: 只抽出目前兩邊已共有的 UI 表面，不做提前擴充
- [Trade-off] 寬度／高度仍由外層控制 → 後續 UX change 可一次改殼內樣式，外層尺寸仍可分頁微調

