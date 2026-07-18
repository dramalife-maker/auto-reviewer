## 1. Backend：載入 pending observations

- [x] 1.1 實作「資料來源：掃描檔案系統 `_pending/`，再 join `mr_reviews`」：依 person `display_name` 掃描各專案 `_pending/`，解析 `mr-{iid}-round-{round}.md`，join `mr_reviews` 得到 status／mr_title；不符檔名或讀檔失敗略過並 warn（「檔名解析失敗」）。驗證：單元測試或整合測試涵蓋 draft+published、缺檔略過、orphan→unknown。
- [x] 1.2 依「片段欄位形狀」與「API：擴充既有 `GET /api/people/:id/reports/latest`」，在 `LatestReportItem` 加入必填 `pending_observations`（排序：published→draft→ignored→unknown，同 status 依 mr_iid／review_round 升序）。滿足 Requirement: Latest reports include pending MR observation snippets。驗證：`cargo test` 中 `report_reader` 相關測試通過 Scenario「Draft and published snippets both appear」「Consumed snippet is omitted」「Orphan snippet is marked unknown」「Empty pending directory yields empty array」。

## 2. Frontend：報告閱讀器顯示

- [x] 2.1 依「UI：總覽彙整 + 專案 tab 詳文」，更新型別與 `ReportsPage`：總覽／專案 tab 渲染「待折入觀察」區塊（status pill、MR 身分、全文），與待確認分開且無 publish／resolve 操作。滿足 Requirement: Report reader UI shows pending observation snippets。驗證：手動或前端型別檢查確認空陣列不顯示區塊、有資料時分組顯示。

## 3. 收尾驗證

- [x] 3.1 跑 `cargo test --test report_reader`（或專案慣用等價指令）確認新增／既有 report-reader 測試全綠；確認 `pending_items` 行為未回歸。

## 4. 無週報仍顯示 pending

- [x] 4.1 擴充 latest-reports：人物存在即 200；`report_date` 可為 null；對「有 `_pending/` 或剩餘 open pending_items、但無該日週報」的專案追加合成卡。滿足 Scenario「Pending observations without any weekly report」與「Pending observations for a project without a latest-week report」。驗證：對應整合測試通過。
- [x] 4.2 報告閱讀器在僅有合成卡時仍顯示總覽／專案 tab 與「待折入觀察」，不誤顯示「尚無週報」。驗證：前端型別／建置通過；空 `projects` 才顯示尚無週報。
