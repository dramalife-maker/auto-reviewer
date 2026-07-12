## Why

現有前端是 Vite + vanilla TypeScript 的單一巨大 DOM 類別，樣式與導覽不一致，難以對齊新設計稿並維護。設計交接已定稿為 React + Tailwind 高保真重做，需一次換軌並保留全部既有 API 行為。

## What Changes

- 前端技術棧改為 React + TypeScript + Tailwind CSS，以手寫原子元件實作 design tokens
- 以 HashRouter 承載六個主要視圖（控制台、MR 收件匣、報告閱讀器、執行紀錄、專案設定、人員設定）
- Sidebar IA 改為「工作台／設定」分組；報告閱讀器人員選擇改為 nav 內嵌 sub-list
- 未歸戶作者管理從全域 header 面板改到人員設定頁頂部；人員設定 nav 在 count > 0 時顯示 badge
- 排程補跑、強制 MR 掃描等設計稿未畫功能全部保留並接既有 API
- 移除 vanilla `ReviewerApp` / `innerHTML` 渲染路徑與舊 ad-hoc CSS

## Capabilities

### New Capabilities

- `frontend-shell`: SPA shell（React + Tailwind design system、HashRouter、sidebar IA、全域 banner、六頁導覽契約）

### Modified Capabilities

- `people-settings`: 未歸戶入口改為人員設定頁內建區塊；移除「header shortcut 必須存在」要求
- `person-identity`: 前端未歸戶管理 UI 位置與入口改為人員設定，行為契約不變

## Impact

- Affected specs: frontend-shell（新）, people-settings, person-identity
- Affected code:
  - New: frontend/src/main.tsx, frontend/src/App.tsx, frontend/src/index.css, frontend/src/components/ui/, frontend/src/components/layout/, frontend/src/pages/, frontend/src/hooks/, frontend/src/lib/icons.ts, frontend/tailwind.config.js, frontend/postcss.config.js
  - Modified: frontend/package.json, frontend/package-lock.json, frontend/vite.config.ts, frontend/tsconfig.json, frontend/index.html, frontend/src/api.ts, frontend/src/types.ts, frontend/src/config.ts
  - Removed: frontend/src/app.ts, frontend/src/style.css, frontend/src/main.ts
