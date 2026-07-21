## Why

Agent Chat 的浮動按鈕（FAB）與展開後的聊天面板目前固定在畫面右下角，可能遮擋使用者正在操作的內容（例如 MR diff 或報告區塊）。使用者希望能拖曳調整位置，並記住位置以避免每次重新整理都要再移動一次。

## What Changes

- 收合狀態的浮動按鈕（FAB）可透過拖曳調整位置。
- 展開後的聊天面板可透過標題列（header）拖曳調整位置。
- 兩者的位置分別持久化到 `localStorage`（FAB 與面板各自獨立存放，因為兩者尺寸與慣用位置不同）。
- 拖曳位置會被限制在目前的 viewport 範圍內，且在視窗尺寸改變（resize）時重新夾限，避免拖出畫面外或被裁切。
- 拖曳與點擊互斥：偵測到實際拖曳位移後，放開滑鼠不會觸發原本點擊即展開／視為誤觸的行為。

## Non-Goals

- 不支援觸控裝置的多指手勢或慣性滑動效果，僅處理單指/滑鼠的基本拖曳。
- 不提供「重置為預設位置」的 UI 按鈕；如需重置，使用者需自行清除瀏覽器 `localStorage`。
- 不支援跨裝置／跨瀏覽器同步位置（僅存於當前瀏覽器的 `localStorage`）。
- 不改變 FAB／面板的開合、已讀狀態、訊息時間戳等既有行為。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `agent-chat-panel`: 浮動按鈕與展開後的面板新增可拖曳定位能力，拖曳結果持久化並在 viewport 內夾限。

## Impact

- Affected specs: agent-chat-panel
- Affected code:
  - New: frontend/src/lib/useDraggablePosition.ts
  - Modified: frontend/src/components/AgentChatLauncher.tsx
  - Modified: frontend/src/components/AgentChatPanel.tsx
  - Modified: frontend/src/components/ui/Card.tsx
  - Modified: frontend/src/components/AgentChatLauncher.test.tsx

