## Context

Agent Chat 目前透過共用元件 `AgentChatLauncher` 提供浮動按鈕（FAB）與展開後的聊天面板（`AgentChatPanel`），兩者皆為 `position: fixed`，固定錨定在 `right`/`bottom` 座標。使用者要求兩者可拖曳調整位置，且拖曳後的位置需要記住。

先前已知的相關踩雷（見 `.claude/docs/learning-notes/2026-07-20/fixed-position-h-full-height-trap.md`）：`position: fixed` 元素的高度／定位計算仍遵循一般 CSS box 規則，`fixed` 只決定「相對哪個祖先定位」，不會自動繼承 viewport 的其他度量。本次設計延續同一批元件，需注意類似的座標系混淆風險（例如 pointer 座標是 viewport 相對，而元件定位用的是 `right`/`bottom`，兩者需要正確換算）。

## Goals / Non-Goals

**Goals:**

- FAB（收合狀態）可透過拖曳調整位置。
- 展開後的面板可透過標題列（header）拖曳調整位置。
- 兩者的位置分別持久化到 `localStorage`，重新整理頁面後維持在使用者上次拖曳的位置。
- 拖曳結果限制在目前 viewport 範圍內；視窗尺寸改變時重新夾限，避免元件被拖出可視範圍或半截卡在畫面外。
- 拖曳與點擊互斥：實際發生位移的拖曳，放開滑鼠不應觸發原本的點擊行為（FAB 展開）。

**Non-Goals:**

- 不支援觸控裝置的多指手勢或慣性滑動效果，僅處理單指/滑鼠 pointer 事件的基本拖曳。
- 不提供「重置為預設位置」的 UI 按鈕。
- 不支援跨裝置／跨瀏覽器同步位置。
- 不改變 FAB／面板既有的開合、已讀狀態、訊息時間戳等行為。

## Decisions

### 使用 Pointer Events 而非 mousedown/touchstart 分別處理

`PointerEvent`（`onPointerDown`/`onPointerMove`/`onPointerUp`/`onPointerCancel`）統一處理滑鼠與觸控輸入，避免重複寫兩套事件邏輯。搭配 `setPointerCapture`/`releasePointerCapture`，確保拖曳中滑鼠移出元件邊界仍能持續收到 move 事件。

替代方案（`mousedown`/`mousemove`/`mouseup` + 額外 `touchstart` 系列）需要維護兩套幾乎相同的邏輯，且需自行處理 pointer capture 的等價行為，複雜度較高，故不採用。

### 以 `right`/`bottom` 位移量而非 `left`/`top` 座標儲存位置

元件既有的 CSS 定位方式是 `right`/`bottom`（錨定在畫面右下角），拖曳時計算 pointer 的位移量（`deltaX`/`deltaY`），並將其反向套用到 `right`/`bottom`（`right -= deltaX`、`bottom -= deltaY`），維持與現有 CSS 定位方式一致，不需改動元件原本錨定右下角的預設樣式邏輯。

替代方案（記錄 `left`/`top` 絕對座標）需要同時改寫元件的定位屬性（從 `right`/`bottom` 改成 `left`/`top`），影響既有預設值與其他未拖曳過的使用者的初始位置，故不採用。

### 位置持久化用獨立的 `localStorage` key（FAB 與面板分開）

FAB 與展開後的面板尺寸差異大（`56px` 圓形按鈕 vs. 動態尺寸的面板），拖曳慣用位置通常不同（例如 FAB 常放邊緣角落，面板需要更靠近使用者正在看的內容）。用兩個獨立 key（`agent-chat-fab-position`、`agent-chat-panel-position`）分別儲存，避免共用一組座標造成其中一個尺寸不合理。

替代方案（共用一組位置）在 FAB 收合/展開切換時會需要額外轉換座標系（因為兩者尺寸不同、夾限範圍不同），複雜度更高且行為較不直覺，故不採用。

### 拖曳/點擊互斥用「位移閾值 + ref 旗標」判斷，而非阻止預設事件

用 `DRAG_THRESHOLD_PX = 4`px 的位移閾值，搭配 `movedRef` 旗標記錄「是否已發生真正拖曳」。`pointerup` 後，呼叫端（`AgentChatLauncher`）在 `onClick` 中檢查 `wasDragged()`，若為 true 則忽略該次點擊事件，不觸發展開。

替代方案（在 `pointerdown` 就呼叫 `event.preventDefault()`/`stopPropagation()` 完全吃掉後續 click）會連帶吃掉合法的單純點擊（因為 `click` 事件在 `pointerup` 後才觸發，且與 `pointerdown`/`pointerup` 屬於不同事件），必須靠位移量判斷「這次操作究竟是點擊還是拖曳」，用旗標比用事件阻擋更可控、更貼合使用者實際意圖。

### Viewport 夾限與 resize 重新夾限

`clamp()` 函式將 `right`/`bottom` 限制在 `[0, window.innerWidth - elementWidth]` / `[0, window.innerHeight - elementHeight]` 範圍內，確保元件不會被拖出可視範圍。監聽 `window resize` 事件，在視窗尺寸縮小時（例如瀏覽器縮小視窗、或裝置旋轉）重新計算並套用夾限，避免元件卡在畫面外或被裁切。

## Implementation Contract

**行為（Behavior）：**

- 使用者用滑鼠／觸控在 FAB 或展開面板的標題列上按下並拖曳，元件應跟隨游標移動；放開後停留在該位置。
- 重新整理頁面或切換路由後，FAB 與面板（若曾被拖曳過）應恢復到使用者上次放開的位置，而不是回到預設右下角。
- 若使用者只是單純點擊 FAB（沒有明顯位移），仍應正常展開 Agent Chat 面板；反之若剛完成一次拖曳（有明顯位移），放開滑鼠後緊接的點擊事件不應觸發展開。
- 若視窗尺寸縮小導致已儲存的位置超出新的可視範圍，元件應自動被拉回到視窗邊界內，而不是停留在畫面外或被裁切。

**介面／資料形狀：**

- 新增 hook：`useDraggablePosition(storageKey: string, elementRef: React.RefObject<HTMLElement | null>)`，回傳 `{ position: DragPosition | null, dragHandleProps: { onPointerDown, onPointerMove, onPointerUp, onPointerCancel }, wasDragged: () => boolean }`。
- `DragPosition` 資料形狀：`{ right: number; bottom: number }`（單位為 px）。
- `localStorage` 儲存格式：`JSON.stringify({ right, bottom })`，key 分別為 `agent-chat-fab-position`（FAB）與 `agent-chat-panel-position`（展開面板）。
- `AgentChatPanel` 新增可選 prop `headerProps?: React.HTMLAttributes<HTMLDivElement>`，供呼叫端（`AgentChatLauncher`）將拖曳事件處理器與 `cursor-move` 樣式附加到標題列，`AgentChatPanel` 本身不需知道拖曳邏輯的存在。
- `ui/Card` 元件需支援 `ref`（`forwardRef`）與 `style` prop，以套用拖曳後計算出的 `right`/`bottom` 內嵌樣式。

**失敗模式：**

- `localStorage` 寫入失敗（例如隱私模式配額限制）時，靜默忽略（不擲出例外、不顯示錯誤訊息），拖曳仍在當前 session 中生效，僅無法持久化到下次載入。
- `localStorage` 讀取到格式不符的資料（非物件、缺少 `right`/`bottom` 數值欄位）時，視為沒有已儲存的位置，回退使用預設 CSS 定位（不擲出例外）。

**驗收標準：**

- `frontend/src/components/AgentChatLauncher.test.tsx` 涵蓋：拖曳 FAB 後位置改變且寫入對應的 `localStorage` key；拖曳後緊接點擊不會展開面板；未拖曳的單純點擊仍會展開面板；拖曳展開面板的標題列後寫入對應的 `localStorage` key。
- `npm run build`（`tsc && vite build`）與 `npx vitest run` 全數通過。

**範圍界線：**

- 範圍內：FAB 與展開面板的拖曳互動、位置持久化、viewport 夾限、拖曳/點擊互斥判斷。
- 範圍外：面板內部的訊息氣泡、composer、開合狀態邏輯、非拖曳相關的既有行為（例如已讀、時間戳顯示）——這些不在本次變更範圍內，不應被觸碰。

## Risks / Trade-offs

- [風險] 拖曳後的位置可能因為畫面配置變化（例如側邊欄展開/收合改變可用寬度）而顯得不合理，但目前的 viewport 夾限僅保證「不超出畫面」，不保證「避開其他 UI 元素」。 → 緩解：可視為可接受的已知限制（Non-Goals 已排除自動避讓其他元素），使用者可再次拖曳調整。
- [風險] `localStorage` 在無痕模式或瀏覽器設定停用儲存時可能寫入失敗。 → 緩解：讀寫皆包在 try/catch 中靜默失敗，退回預設定位，不影響核心功能（開合、傳送訊息）。
- [風險] 兩個獨立的 `localStorage` key 若未來因為多分頁同時操作可能互相覆蓋（無跨分頁同步機制）。 → 緩解：此為可接受的已知限制，非本次目標；多分頁情境下以最後寫入的分頁位置為準。
