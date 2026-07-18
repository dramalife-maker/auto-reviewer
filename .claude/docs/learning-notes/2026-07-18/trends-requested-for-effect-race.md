# 競態條件：`useEffect` 內先 `set` 又把同一 state 當 dependency → cancel 後 loading 永遠不關

<tl;dr>
- **何時要想起這則：** React 裡用「已請求標記」避免重複 fetch，且標記是 `useState`、又列在同一個 `useEffect` 的 dependency 裡時。
- **不要做：** 在 kick off async 之前就 `setRequestedFor(id)`（或任何會立刻觸發本 effect 重跑的 set）。
- **要做：** 請求**結束後**再標記；或改用 `useRef` 當已請求旗標，不要把旗標放進 dependency。
- **症狀：** UI 永遠停在「載入中…」；Network 其實已 200，但 state 沒寫入／loading 沒清掉。
- **自問（可選）：** 這個 setState 會不會讓本 effect cleanup 先跑、把 `cancelled=true`？
</tl;dr>

## 使用者為何希望這樣改（意圖）

報告閱讀器「成長趨勢」分頁應在點選後載入 `_people/{name}/` 的長期觀察與月度時間軸。

## 問題描述

選人後點「成長趨勢」，畫面一直顯示「載入成長趨勢中...」。後端 `GET /api/people/:id/trends` 正常、磁碟上 `index.md`／`YYYY-MM.md` 也有資料。

## 錯誤原因／學到的知識

原寫法（簡化）：

```ts
useEffect(() => {
  if (activeTab !== 'trends' || trendsRequestedFor === personId) return

  let cancelled = false
  setTrendsRequestedFor(personId) // ← 立刻改 dependency
  setTrendsLoading(true)
  fetchPersonTrends(personId)
    .then(...)
    .finally(() => {
      if (!cancelled) setTrendsLoading(false)
    })

  return () => {
    cancelled = true
  }
}, [activeTab, personId, trendsRequestedFor])
```

時序：

1. Effect 跑：`setTrendsRequestedFor(personId)` + 開始 fetch
2. `trendsRequestedFor` 變更 → React 重跑 effect → **先跑 cleanup** → `cancelled = true`
3. 新一輪 effect：`trendsRequestedFor === personId` → **early return**，不再開新請求
4. 舊 fetch 完成：`finally` 見 `cancelled` → **不** `setTrendsLoading(false)`，也不寫入 `trends`

結果：loading 永遠為 true。API／資料都沒問題，是 effect 自己 cancel 掉收尾。

本質：**「請求前寫入、且該值又是本 effect 的 dependency」** 等於保證 cleanup 會在 settle 前記一次。

## 解決方法

在 settle 後再標記（成功／失敗都可）：

```ts
.finally(() => {
  if (!cancelled) {
    setTrendsRequestedFor(personId)
    setTrendsLoading(false)
  }
})
```

這樣 dependency 變更發生在請求結束之後，不會取消「正在飛的那次」的收尾。

替代：`useRef` 記已請求的 `personId`，effect 只依 `activeTab`／`personId`，不動 ref-as-dependency。

## 避免方法

- 凡「防重複請求」旗標：優先 `ref`；若用 state，**不要**在 start 時 set 並列入同一 effect deps。
- Review checklist：effect 裡有 `cancelled` + 有 `setState` 改到 deps 嗎？有 → 模擬 Strict Mode／該 set 觸發的重跑。
- 卡住「載入中」時先對 Network：已 200 仍轉圈 → 先查 effect cancel／early-return，再查 API。

## 相關檔案

- `frontend/src/pages/ReportsPage.tsx`（成長趨勢 `useEffect`／`trendsRequestedFor`）
