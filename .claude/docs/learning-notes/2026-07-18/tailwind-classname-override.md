# 知識缺口：Tailwind 衝突以 stylesheet 順序為準，後寫的 `className` 不保證覆蓋

<tl;dr>
- **何時要想起這則：** 共用 atom（如 `Button`）內建 `rounded-md`／`px-*`，呼叫端想用 `className` 改成 `rounded-full` 或清掉 padding 時。
- **不要做：** 以為 JSX 裡 `className` 字串寫在後面就一定蓋過元件內建 utility。
- **要做：** 需要覆蓋時用 `!rounded-full`／`!p-0`，或改 atom 支援 variant／用 `tailwind-merge`，或不用該 atom 直接寫原生 button。
- **症狀：** class 字串看起來有 `rounded-full`，畫面仍是圓角矩形。
- **自問（可選）：** DevTools 裡真正生效的是哪一條 `border-radius`？有沒有同屬性的另一個 utility 贏了？
</tl;dr>

## 使用者為何希望這樣改（意圖）

Agent Chat FAB 要做成正圓；元件上已加 `rounded-full`，但畫面仍偏方形。

## 問題描述

`AgentChatLauncher` 的 FAB 使用共用 `Button`，並傳入：

```tsx
className="... size-14 rounded-full px-0 ..."
```

`Button` 本體則固定帶 `rounded-md`（以及 `px-[18px] py-2.5`）。結果仍是圓角方形，不是正圓。

## 錯誤原因／學到的知識

Tailwind（含 v4）對**互相衝突的 utility**（例如 `rounded-md` vs `rounded-full`）的勝負，取決於**產生出的 CSS 在 stylesheet 裡的順序**，不是 HTML `class` 屬性字串誰寫在後面。

因此這種寫法：

```tsx
// Button 內部
className={['... rounded-md ...', className].join(' ')}
```

呼叫端即使在 `className` 再傳 `rounded-full`，**不保證**蓋過內建的 `rounded-md`。

## 解決方法

FAB 這次用 important 覆蓋（範圍小、不改全域 Button API）：

```tsx
className="... !size-14 !rounded-full !p-0 ..."
```

若同類需求變多，較乾淨的做法是：

- atom 提供 `shape`／`size` variant，或
- 用 `tailwind-merge` 合併 class，讓後傳入的衝突 utility 真正勝出。

## 避免方法

- 改 atom 外觀時，先在 DevTools 確認生效的是哪條 rule，不要只看 class 字串。
- Review：共用元件若 bake 了 layout／shape utility，呼叫端覆寫要嘛 important／merge，要嘛改 atom，不要假設字串順序。

## 相關檔案

- `frontend/src/components/ui/Button.tsx`
- `frontend/src/components/AgentChatLauncher.tsx`（FAB）
