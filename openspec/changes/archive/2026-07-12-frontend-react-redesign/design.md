## Context

現有前端為 Vite + vanilla TypeScript，狀態與渲染集中在單一 `ReviewerApp` 類別（約 2500 行 `innerHTML`），樣式為 ad-hoc CSS。設計交接 `docs/design_handoff_reviewer_redesign` 已定稿為 React + Tailwind 高保真重做，並重組 sidebar IA。後端 API 與 `frontend/src/api.ts` / `types.ts` 契約不變。

相關既有能力：people-settings、person-identity、report-reader、scheduling、run-history。本變更新增 frontend-shell，並修改未歸戶 UI 入口需求。

## Goals / Non-Goals

**Goals:**

- 一次換軌為 React + TypeScript + Tailwind，刪除 vanilla 渲染路徑
- 對齊 design handoff 的 tokens、六頁版面與 sidebar IA
- HashRouter 重整可回到同一視圖
- 保留未歸戶、排程補跑、強制 MR 掃描，並接到既有 API
- 未歸戶管理改到人員設定頁

**Non-Goals:**

- 後端 API / schema 變更
- 響應式布局低於約 1024px
- 引入 shadcn 或其他元件庫、全域狀態庫（Redux 等）
- 照抄 design prototype 的 runtime 或 inline-style 寫法

## Decisions

### 一次換軌而非長期雙軌

採用單一 React 應用取代 vanilla，實作仍分階段（工具鏈 → shell → 六頁），但不並存兩套 UI。避免樣式與事件綁定雙軌維護成本。

### 手寫 Tailwind 原子元件

不引入 shadcn。Design tokens（indigo primary、MR violet、扁平無 shadow、inset selection）與預設元件庫差異大，手寫 Button / Card / Badge / Tabs / StatCard / ListRow 等較可控。

### HashRouter 承載六視圖

使用 `react-router-dom` 的 `HashRouter`，與現有 `VITE_BASE_PATH` 相容。路由：`#/dashboard`、`#/mr-inbox`、`#/reports/:personId?`、`#/runs/:runId?`、`#/projects`、`#/people`。MR 篩選以 query `status` 表示。

### 未歸戶入口改到人員設定

人員設定頁頂部顯示未歸戶綁定區塊；sidebar「人員設定」在 count > 0 時顯示 badge。移除全域 header 未歸戶面板與「header shortcut 必須存在」需求。

### 保留 api.ts / types.ts

頁面與 hooks 繼續呼叫既有 fetch 函式，不改後端契約。執行中 run polling、MR dirty editor、schedule catch-up sessionStorage dismiss 等行為從 `ReviewerApp` 搬到對應 hooks。

### 設計稿未涵蓋功能的安置

- 排程補跑：留在控制台排程面板（維持 scheduling spec 的 dashboard banner / session dismiss 行為）
- 強制 MR 掃描：專案設定 detail 次要 action
- 版本字與連線狀態：sidebar 品牌區與 footer

## Implementation Contract

**Behavior**

- 操作者開啟前端後看到固定 232px sidebar（工作台／設定分組）與主內容區
- 透過 hash 路由在六頁間導覽；重整後仍停留在同一 hash
- 報告閱讀器人員在 sidebar 展開列表選擇；人員設定內可綁定未歸戶
- 控制台可編輯／儲存排程、觸發立即執行與補跑；專案可執行／掃描 MR（含強制）；MR 收件匣可編輯草稿／發布／忽略／agent chat；執行紀錄可檢視 run 與 skip 摘要

**Interface / data shape**

- 繼續使用既有 REST 端點與 `api.ts` 回傳型別（DashboardResponse、PersonDetail、MrReviewItem、RunStatus、UnmatchedAuthor、ScheduleUpdateInput 等）
- Hash 路徑契約如上 Decisions；MR filter：`#/mr-inbox?status=draft|published|ignored`
- Catch-up dismiss：`sessionStorage`，key 依 `due_at`（行為對齊既有 scheduling spec）

**Failure modes**

- 後端連線失敗：顯示錯誤狀態／banner，不得靜默空白到無法操作
- 個別頁面 fetch 失敗：頁內錯誤或全域 banner（錯誤樣式），其餘已載入資料可保留
- Run / MR 衝突類錯誤：以 banner 顯示後端訊息

**Acceptance criteria**

- `npm run build`（frontend）成功
- 無 `ReviewerApp` / `innerHTML` 主渲染路徑
- 手動回歸：六頁導覽、hash 重整、未歸戶綁定、補跑與 session dismiss、強制掃描、run polling、MR 發布／忽略
- Spectra tasks 全部勾選；`spectra validate frontend-react-redesign` 通過

**Scope boundaries**

- In scope：frontend 工具鏈、shell、六頁 UI、未歸戶 IA 遷移、設計 token 原子元件
- Out of scope：backend、資料庫、CLI reviewer agent、低於 1024px 響應式

## Risks / Trade-offs

- [大型 UI 重寫遺漏行為] → 以 tasks 按頁列出 parity 清單；對照現有 api 呼叫與 scheduling／person-identity specs 手動回歸
- [Hash 與 base path 互動] → 使用 HashRouter；沿用現有 vite proxy 與 `normalizeBasePath`
- [時間顯示 UTC 誤解] → 沿用既有學習筆記：SQLite UTC 字串解析時不得當本地時區

## Migration Plan

1. 新增 React／Tailwind 依賴與目錄骨架，仍可本地 `npm run dev`
2. 實作 shell＋路由後逐頁替換，完成後刪除 `app.ts`／`style.css`／`main.ts`
3. Rollback：還原 frontend 目錄至變更前 commit（無後端遷移）

## Open Questions

（無 — 遷移策略、未歸戶 IA、HashRouter、手寫 Tailwind、Spectra 流程已在計劃對齊階段鎖定）
