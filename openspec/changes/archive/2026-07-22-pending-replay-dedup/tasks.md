## 1. 插入邏輯

- [x] 1.1 實作 Weekly summary ingestion deduplicates open pending questions 的重放防護：在 summary.rs 的待確認插入處，改以單一原子語句加上 `WHERE NOT EXISTS` 子查詢，條件為同一 person_id、project_id、question 且其來源報告日期大於或等於本次 summary 的報告日期；`INSERT OR IGNORE` 保留，繼續由 idx_pending_open_unique 負責擋下同題雙 open。驗證：`cargo test -p reviewer-server --test pending_items` 全綠，且既有三項測試 reingesting_same_summary_does_not_duplicate_pending_item、carrying_open_question_into_next_week_does_not_duplicate_pending_item、resolved_question_may_be_raised_again_as_new_open_row 未經修改即通過。
- [x] 1.2 處理來源報告缺失情形：當既有列的 report_id 為 NULL 而取不到報告日期時，不阻擋插入並記錄一則含 person_id、project_id、question 的 warn。驗證：新增測試建立 report_id 為 NULL 的既有列，斷言 ingest 後新列成功建立。

## 2. 測試

- [x] 2.1 新增測試涵蓋 Scenario: Re-reading an already-processed summary creates no row：問題已 resolved 後，重新 ingest 同一份報告日期的 summary，斷言 pending_items 總列數不變。驗證：測試名稱明確描述重放情境，執行後為綠。
- [x] 2.2 新增測試涵蓋 Scenario: Re-reading an older summary creates no row：問題於較新報告日期被解決後，ingest 一份較舊報告日期且含同題待確認的 summary，斷言不新增列且該題未回到 open。驗證：斷言同時檢查總列數與 open 列數。
- [x] 2.3 [P] 新增測試涵蓋跨週攜帶後解決再重放的路徑：問題於 D1 提出、於 D2 攜帶（report_id 被更新為 D2 的報告）、隨後解決，接著重放 D1 的 summary，斷言不產生重複列。此案例即為採用大於等於而非等號比較的理由。驗證：測試在比較改為等號時會失敗，確認其確實鎖住該行為。

## 3. 正式環境清理

- [x] 3.1 清除正式環境重複的 pending_items 列：刪除 person_id 2、project_id 1 底下與既有列問題字面相同的較新一列，保留最早建立者。驗證：清理後以查詢確認該 (person_id, project_id, question) 組合僅剩一列，且 pending_items 總列數符合預期。
- [x] 3.2 清除人員筆記檔中的重複紀錄行：移除 Power 筆記檔中與另一行完全相同的已解決紀錄，保留一行。驗證：以內容檢視確認該問題僅出現一次，且檔案其餘行未被更動。

## 4. 整體驗證

- [x] 4.1 確認無回歸：執行 `cargo test -p reviewer-server` 與 `cargo clippy --all-targets`。驗證：測試全綠、clippy 無新增警告；已知既有失敗 mr_scan_timeout_still_ingests_draft_on_disk 與 app-env crate 的三項測試不計入，但須確認其失敗訊息與本變更前相同。
