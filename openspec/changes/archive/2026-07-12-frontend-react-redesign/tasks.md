## 1. 工具鏈與 design system

- [x] 1.1 依「一次換軌而非長期雙軌」與「手寫 Tailwind 原子元件」決策，為 frontend 安裝 React、react-dom、react-router-dom、@vitejs/plugin-react、Tailwind／PostCSS，並設定 theme tokens（indigo／MR violet／扁平無 shadow）；驗證：`npm run build` 在空 React mount 下可通過
- [x] 1.2 [P] 實作 ui atoms（Button、Card、Badge、Input、NavItem、StatCard、ListRow、Tabs、StatusPill、Avatar）與 GitLab／folder icon data-URI，使「Frontend uses React shell with Tailwind design system」可被頁面複用；驗證：Story-less 手動渲染或頁面引用時 class 對齊 design tokens（無 drop shadow）

## 2. Shell 與路由

- [x] 2.1 依「HashRouter 承載六視圖」實作 HashRouter 與六頁 outlet，滿足「Hash routes map to the six primary views」（含 `#/reports/:personId`、`#/runs/:runId`、`#/mr-inbox?status=`）；驗證：手動切換 hash 與重整後仍停留在同一視圖
- [x] 2.2 實作 AppShell／Sidebar，滿足「Sidebar navigation groups workbench and settings」（工作台／設定分組、MR violet badge、報告人員 sub-list、連線狀態來自 `fetchHealth`）；驗證：點人員列導向 `#/reports/:personId`，pending badge 僅在 open pending > 0 顯示

## 3. 資料層與共用行為

- [x] 3.1 依「保留 api.ts / types.ts」決策，以 hooks 包裝既有 `api.ts` 呼叫（dashboard、projects、people、mr-reviews、runs、unmatched、schedule），不改後端契約；驗證：TypeScript 編譯通過且函式簽名仍對應既有 types
- [x] 3.2 [P] 實作全域 Banner 與 run polling／錯誤顯示慣例，使 API 失敗與成功訊息可被操作者看見；驗證：模擬失敗時出現 error banner，dismiss 後可關閉

## 4. 六頁遷移

- [x] 4.1 遷移 Dashboard（stats、最近報告／執行、全寬排程、立即執行），並依「設計稿未涵蓋功能的安置」接上 catch-up／sessionStorage dismiss，滿足「Feature parity for non-prototype actions」中的補跑；驗證：有 `missed_weekly_run` 時可補跑與同 tab dismiss（對齊 scheduling spec）
- [x] 4.2 [P] 遷移 Runs History（列表＋detail、auto-fit meta、MR skip 紅卡）；驗證：選 run 後 detail 顯示專案結果與 skip 摘要
- [x] 4.3 [P] 遷移 Project Settings（260px list＋detail、hover 執行、掃描 MR＋強制掃描）；驗證：強制掃描呼叫 force API，滿足 feature parity
- [x] 4.4 遷移 People Settings，滿足「People settings UI manages persons and identities」與「Frontend exposes unmatched author management」，並依「未歸戶入口改到人員設定」在頁頂放未歸戶區塊、nav badge；驗證：綁定後 unmatched 減少且無 header 未歸戶面板
- [x] 4.5 [P] 遷移 Reports Reader（max-width 800px、總覽／專案／成長趨勢 tabs、pending／已讀／完整 md）；驗證：從 sidebar 選人後內容與 tabs 正確切換
- [x] 4.6 [P] 遷移 MR Inbox（篩選 tabs、選取 inset、textarea 草稿、發布／忽略／agent chat，violet 僅限此 track）；驗證：儲存／發布／忽略與 agent turn 呼叫既有 API

## 5. 清理與驗收

- [x] 5.1 刪除 vanilla 入口（`app.ts`、`style.css`、`main.ts`），確認主路徑僅 React，完成一次換軌；驗證：repo 內無 `ReviewerApp` 作為 UI 入口，`npm run build` 成功
- [x] 5.2 手動回歸六頁＋hash 重整＋未歸戶＋補跑＋強制掃描＋run polling＋MR 流程；驗證：對照 frontend-shell／people-settings／person-identity scenarios 全部可觀察通過
