## 1. Runs list and detail APIs

- [x] 1.1 實作 Runs list API／Runs history list API（列表分頁與篩選從簡）：`GET /api/runs` 分頁（limit 預設 50、最大 200）、可選 trigger／status 篩選，依 `started_at` 降序回傳 `runs`+`total`。驗證：整合測試覆蓋排序、分頁、篩選、非法 limit → 400。
- [x] 1.2 實作 Run detail with skip summary／Run detail includes timing and MR skip summary（明細擴充既有 GET，不另開路徑；Skip 摘要讀 `eligible_mrs.json`，不建表）：擴充 `GET /api/runs/{id}` 補 `duration_sec`／`note`、專案時間欄位；MR trigger 從 `eligible_mrs.json` 組 per-project `skip_summary`（by_reason + items≤100）；缺檔回空摘要。驗證：整合測試覆蓋失敗專案欄位、MR skip 摘要、缺檔空摘要。

## 2. Dashboard recent runs

- [x] 2.1 實作 Dashboard includes recent runs（Dashboard 最近 5 筆）：`GET /api/dashboard` 附最多 5 筆 `recent_runs`。驗證：更新 dashboard 整合測試斷言長度與排序。

## 3. Execution history UI

- [x] 3.1 實作 Runs UI／Execution history UI with dashboard entry（控制台入口 + AppView `runs`，側欄可進）：新增 AppView `runs`、側欄入口、列表、明細（含 MR skip 分組）；控制台最近執行可點進。驗證：`npm run build`；手動確認列表／明細／skip 顯示。

## 4. Docs

- [x] 4.1 更新 `README.md` 與 `docs/idea/schema.md`：記載 runs 列表／明細契約與 skip_summary 來源。驗證：文件內容審查含端點與欄位說明。



