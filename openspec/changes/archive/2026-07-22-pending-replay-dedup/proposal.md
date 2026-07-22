## Problem

每次 run 執行完的 ingest 階段會掃描該專案報告目錄底下**所有**歷史 `summary.md`，不只本次產生的那一份。重放舊檔時，其 `## 待確認` 條目會被重新插入 `pending_items`，在同一個問題已經是 `resolved` 的情況下產生重複列。

正式環境已觀察到：同一個問題存在 item 19 與 item 25 兩列（字面完全相同，皆 resolved），後者由 run 41 重放 2026-07-12 的 summary 產生。每執行一次該專案的 run 就多一列，沒有上界。

傷害不只是資料膨脹：

- `resolve_pending_item` 會同步寫入人員層級的 `_notes.md`，而 `apply_resolved_line` 找不到對應 open 行時採 append，因此筆記檔同樣累積重複行。正式環境的 Power 筆記檔已有兩行完全相同的紀錄。
- 該筆記檔透過 `notes_dir` 餵給 reviewer agent 當上下文，重複內容會消耗 token 並可能干擾判斷。
- `find_summary_files` 使用 `read_dir`，順序不保證。若某次重放先處理較新的 `## 已釐清` 再處理較舊的 `## 待確認`，已關閉的問題會停留在 `open`，重新出現在待確認清單上。

## Root Cause

`pending_items` 是整條 ingest 路徑上唯一不冪等的資料表。

`reports` 使用 `ON CONFLICT(project_id, person_id, report_date) DO UPDATE`，重放幾次結果都相同。但 `pending_items` 的插入依賴 `idx_pending_open_unique`，該索引帶有 `WHERE status = 'open'` 的部分條件。問題一旦被解決就退出索引覆蓋範圍，`INSERT OR IGNORE` 沒有可忽略的對象，於是照常插入新列。

現行規格《Weekly summary ingestion deduplicates open pending questions》明文允許「已解決的問題可以再次被提出」，這是刻意設計並有測試鎖住。缺的不是這條規則本身，而是它沒有區分兩種來源相同、意義不同的情境：

- 重放：舊 summary 被重新讀取，其中的問題早已處理完畢
- 真實重提：新的 summary 再次提出同一個問題

## Proposed Solution

以**來源報告日期**作為判別依據。`pending_items.report_id` 可外連 `reports.report_date`，取得完整日期而非僅有月份。

插入 `## 待確認` 條目前，若同一 `(person_id, project_id, question)` 已存在任何一列（不分狀態），且該列的來源報告日期 **大於或等於** 本次傳入 summary 的報告日期，則跳過插入。

語意為：這個問題在該時間點或更晚已經被記錄過，更舊的提及屬於歷史而非新事件。

採用 `>=` 而非等號，是為了涵蓋「問題跨週攜帶時 `report_id` 被更新為較新報告 → 之後被解決 → 重放更舊的 summary」這條路徑；僅比對等號會在此漏接。

`idx_pending_open_unique` 與 `INSERT OR IGNORE` 均保留，繼續負責「同一問題不得同時有兩列 open」。新的檢查是疊加而非取代。

當 `report_id` 為 `NULL`（來源報告已刪除，欄位為 `ON DELETE SET NULL`）而無法取得日期時，不阻擋插入並記錄一則警告，避免靜默吞掉真正的新問題。

## Non-Goals

- **不縮小 ingest 掃描範圍**：曾評估只處理本次 run 產生的報告，但既有測試以「改寫同一份 summary 再重新 ingest」的方式模擬問題狀態變化，縮小範圍會使三個既有測試失效
- **不改動 schema**：`report_id` 欄位已存在，透過 join 即可取得報告日期，無需新增欄位、索引或追蹤表
- **不移除既有的「已解決問題可再次提出」語意**：該行為刻意設計且有測試鎖住，本變更只為其補上時間維度的限定
- **不寫 migration 清理既有重複資料**：正式環境僅一列重複列與一行重複筆記，且筆記檔位於磁碟，SQL migration 無法處理；改以一次性手動清理
- **不修改 `_notes.md` 的 append 行為**：重複行的根因是重複的 resolve 呼叫，源頭堵住後不再產生

## Success Criteria

- 對同一份 summary 重複執行 ingest，`pending_items` 列數不增加，無論該問題目前為 `open` 或 `resolved`
- 較新的 summary 再次提出已解決的問題時，仍會建立新的 `open` 列
- 較舊的 summary 被重放時，不會建立新列，亦不會使已解決的問題回到 `open`
- 問題跨週攜帶後被解決，再重放更舊的 summary 時不產生重複列
- `report_id` 為 `NULL` 時插入照常進行並留下警告紀錄
- 既有三項測試維持通過：`reingesting_same_summary_does_not_duplicate_pending_item`、`carrying_open_question_into_next_week_does_not_duplicate_pending_item`、`resolved_question_may_be_raised_again_as_new_open_row`
- 正式環境的重複列與重複筆記行完成一次性清理

## Impact

- Affected specs: `pending-closure`
- Affected code:
  - Modified:
    - backend/src/summary.rs
    - backend/tests/pending_items.rs
  - New: (none)
  - Removed: (none)
- 無資料庫 schema 變更
- 需一次性手動清理正式環境資料：`pending_items` 一列重複列，以及人員筆記檔中的一行重複紀錄
