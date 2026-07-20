<tl;dr>
**何時要想起這則：** 設計「填滿容器高度」的 UI（`h-full` / `height: 100%`），尤其是 `position: fixed` 的浮動元件（懸浮聊天視窗、Modal、Toast）想要一個「相對 viewport 比例」的高度時。
**不要做：**
- 假設 `position: fixed` 的元素套用 `h-full` 就會自動吃滿 viewport 高度。
- 讓 `h-full` 的父層只有 `flex flex-col`（沒有明確 `height`）卻期待子層 `h-full` 生效。
- 只在最外層檢查有沒有高度，忽略中間某一層可能斷鏈（沒設 height）。
**要做：**
- 若要「相對 viewport 比例」的高度，直接寫 `h-[min(70vh,560px)]` 這種帶 `vh`/`vw` 單位的值，不要繞道 `h-full`。
- 若真的要用 `h-full`，逐層確認從該元素到最近「有明確 height」的祖先之間，每一層都有設定 height（或用 `absolute`/`fixed` + `inset-0` 搭配明確定位的祖先）。
- 用瀏覽器 DevTools 檢查 computed height，確認沒有某層是 `auto`。
**意圖（建議）：** 使用者想要「聊天視窗盡量利用可用空間，但不超過 70vh/560px 上限」的彈性高度設計；但因外層容器沒有配合設計高度鏈，導致該寫法在 fixed 浮動元件情境下失效，退回固定高度 `min(70vh,560px)` 即可解決且與其他頁面一致。
**自問（可選）：** 這個 `h-full` 的最近祖先鏈中，每一層都有明確 `height` 嗎？`position: fixed` 只影響定位基準，不會讓 `height: 100%` 自動變成相對 viewport。
</tl;dr>

## 使用者為何希望這樣改（意圖）

使用者原本想要「聊天視窗盡量利用可用空間，但不超過 70vh/560px 上限」這種彈性高度設計（`h-full max-h-[min(70vh,560px)]`），但因外層容器沒有配合設計高度鏈，導致該寫法在 fixed 浮動元件情境下失效，視窗高度縮水成內容自身高度。使用者確認後選擇退回固定高度 `min(70vh,560px)`，與其他頁面（`ReportsPage.tsx`）及元件預設值保持一致，優先求正確與一致，而非保留彈性但行為不可預期的寫法。

## 問題描述

專案中 `AgentChatLauncher`（`frontend/src/components/AgentChatLauncher.tsx`）是一個 `position: fixed`（`fixed right-4 bottom-4`）的浮動聊天視窗容器，內部包一層 `Card`（同樣 `fixed`，`flex flex-col`），再放 `AgentChatPanel`。

`AgentChatPanel` 接受一個 `panelClassName` prop 決定聊天視窗高度：

- 預設值 `h-[min(70vh,560px)]`（固定高度，正常運作，`ReportsPage.tsx` 使用此預設值，沒有問題）
- 但在 `MrInboxPage.tsx` 中，開發者把 `panelClassName` 覆寫成 `"h-full max-h-[min(70vh,560px)]"`，意圖是「讓聊天視窗填滿容器高度，但最多不超過 70vh/560px」

結果使用者回報：點開 mr-inbox 頁面的浮動聊天按鈕後，聊天視窗高度明顯偏小（比預期的 70vh/560px 小很多）。

## 錯誤原因（root cause）

`h-full`（`height: 100%`）的計算基準是「最近的父層 box 的 height」。若父層沒有明確設定 height（例如只有 `display: flex; flex-direction: column`，沒有 `height` 或 `flex-basis`），則父層的 height 是 `auto`（由內容撐出），這時子層的 `height: 100%` 無法解析出實際數值，會退化／塌陷成內容自身高度。

在這個案例中，`AgentChatLauncher` 內部的 `Card`（`fixed ... flex w-[min(100vw-2rem,400px)] flex-col overflow-hidden`）只設定了 `width`，**沒有設定 height**，因此 `AgentChatPanel` 上的 `h-full` 沒有可繼承的高度基準，實際渲染高度變成由訊息內容多寡撐出來，而不是預期的 70vh。

額外要注意：即使元素是 `position: fixed`，`height: 100%` 仍然遵循「相對於最近父層的 height」規則，`fixed` 定位只影響「相對於哪個祖先做定位（top/right/bottom/left）」，不會讓 `height: 100%` 自動變成相對於 viewport。這是本次踩雷的知識缺口核心：容易誤以為 fixed 元素的 100% height 會自動吃滿 viewport，但實際上仍需要一個有明確 height 的祖先鏈。

## 解決方法

把 `MrInboxPage.tsx` 中 `panelClassName` 從 `"h-full max-h-[min(70vh,560px)]"` 改回固定高度 `"h-[min(70vh,560px)]"`，與元件預設值、`ReportsPage.tsx` 的用法保持一致。這樣不需要修改父層結構，直接用固定高度取代「填滿父層再限制上限」的寫法。

## 避免方法 / 下次怎麼做

- 當設計「填滿父容器高度」的 UI（`h-full` / `height: 100%`）時，務必往上追祖先鏈，確認每一層都有明確的高度來源（`height: Xpx`、`height: 100vh`、`flex: 1` 且父層本身有高度、或 grid/flex 容器本身有固定尺寸）。只要鏈中斷一層（例如某層是 `flex flex-col` 但沒有指定 height），`h-full` 就會失效。
- 對於 `position: fixed` 的浮動元件（如懸浮聊天視窗、Modal、Toast），如果需要「相對 viewport 的比例高度」，優先直接寫成 `h-[min(70vh,560px)]` 這種明確帶視窗單位（`vh`/`vw`）的值，而不是依賴 `h-full` 搭配某層容器高度——因為 fixed 元件的父層鏈往往是「為了排版方便」而寫的 flex/inline 容器，不會特別去補高度。
- 若真的想用 `h-full`，先確認：這個元素到最近有明確 height 的祖先之間，每一層是否都設了 height（或用 `inset-0`/`absolute` 搭配明確定位的祖先）。可以用瀏覽器 DevTools 逐層檢查 computed height 是否為 `auto`。
