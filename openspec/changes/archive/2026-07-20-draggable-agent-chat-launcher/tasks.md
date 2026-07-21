## 1. 拖曳定位核心邏輯

- [x] 1.1 新增 `useDraggablePosition` hook（`frontend/src/lib/useDraggablePosition.ts`），實作「使用 Pointer Events 而非 mousedown/touchstart 分別處理」：透過 `onPointerDown`/`onPointerMove`/`onPointerUp`/`onPointerCancel` 統一處理滑鼠與觸控拖曳，並用 `setPointerCapture` 確保拖曳中游標移出元素邊界仍持續收到移動事件。驗證：`npx vitest run src/components/AgentChatLauncher.test.tsx` 全數通過。
- [x] 1.2 實作「以 `right`/`bottom` 位移量而非 `left`/`top` 座標儲存位置」：拖曳時計算 pointer 位移量並反向套用到 `right`/`bottom`，維持元件既有錨定右下角的 CSS 定位方式不變。驗證：拖曳 FAB 或面板後，元素的 `style.right`/`style.bottom` 隨拖曳距離正確變化（見 `AgentChatLauncher.test.tsx` 中 `drags the FAB to a new position and persists it` 測試）。
- [x] 1.3 實作「Viewport 夾限與 resize 重新夾限」：`clamp()` 函式將位置限制在 `[0, innerWidth - width]`／`[0, innerHeight - height]` 範圍內，並監聽 `window resize` 事件在視窗縮小時重新套用夾限，滿足 Agent Chat floating button and panel are draggable 需求中「Shrinking the viewport re-clamps an out-of-bounds position」情境。驗證：手動於瀏覽器縮小視窗尺寸，確認已拖曳過的 FAB／面板不會停留在畫面外。

## 2. 拖曳與點擊互斥判斷

- [x] 2.1 實作「拖曳/點擊互斥用「位移閾值 + ref 旗標」判斷，而非阻止預設事件」：以 `DRAG_THRESHOLD_PX = 4` 判斷是否發生真正拖曳，並提供 `wasDragged()` 供呼叫端在 `onClick` 中判斷是否要忽略該次點擊。驗證：`AgentChatLauncher.test.tsx` 中 `does not open the overlay when the FAB drag click follows a real drag` 與 `opens the overlay on a plain click without movement` 兩個測試皆通過。

## 3. 位置持久化

- [x] 3.1 實作「位置持久化用獨立的 `localStorage` key（FAB 與面板分開）」：FAB 使用 `agent-chat-fab-position`、展開面板使用 `agent-chat-panel-position` 兩個獨立 key 分別讀寫，讀寫皆包在 try/catch 中靜默失敗以符合 Implementation Contract 的失敗模式要求。驗證：`AgentChatLauncher.test.tsx` 中 `drags the FAB to a new position and persists it` 與 `drags the expanded panel via its header` 測試分別驗證兩個 key 皆有寫入 `window.localStorage`。

## 4. 元件整合

- [x] 4.1 更新 `AgentChatLauncher.tsx`：FAB 套用 `fabDrag.dragHandleProps` 與計算後的 `style`，`onClick` 檢查 `wasDragged()` 後才觸發展開；展開面板的 `Card` 套用 `panelDrag` 計算後的 `style`。驗證：`Agent Chat floating button and panel are draggable` 需求下四個 scenario（拖曳 FAB 移動並持久化、拖曳面板 header 移動並持久化、完成拖曳不觸發展開、單純點擊仍展開）皆有對應測試通過。
- [x] 4.2 更新 `AgentChatPanel.tsx`：新增可選 `headerProps` prop，讓標題列（含 "Agent Chat" 標題與關閉按鈕的 row）可接收外部傳入的拖曳事件處理器與樣式，`AgentChatPanel` 本身不耦合拖曳邏輯。驗證：`AgentChatLauncher.test.tsx` 中 `drags the expanded panel via its header` 測試透過 `screen.getByText('Agent Chat').parentElement` 取得該 header row 並觸發拖曳事件，成功寫入 `localStorage`。
- [x] 4.3 更新 `ui/Card.tsx` 為 `forwardRef` 並支援 `style` prop，使拖曳後的 `right`/`bottom` 內嵌樣式可套用到面板容器且不影響既有呼叫端（無需傳入 `ref`/`style` 的用法維持不變）。驗證：`npx vitest run src/components/ui/atoms.test.tsx` 與其他既有引用 `Card` 的頁面測試（`DashboardPage`、`PeoplePage`、`ProjectsPage`、`ReportsPage`、`RunsPage`、`MrInboxPage`）全數通過。

## 5. 驗證與收尾

- [x] 5.1 新增拖曳相關測試至 `AgentChatLauncher.test.tsx`：涵蓋拖曳 FAB 持久化、拖曳後點擊不展開、單純點擊仍展開、拖曳面板 header 持久化四個案例。驗證：`npx vitest run src/components/AgentChatLauncher.test.tsx` 顯示 7 個測試全數通過。
- [x] 5.2 確認全專案建置與既有測試未受影響。驗證：`npm run build`（`tsc && vite build`）成功產出，`npx vitest run` 顯示 15 個測試檔、53 個測試全數通過。
