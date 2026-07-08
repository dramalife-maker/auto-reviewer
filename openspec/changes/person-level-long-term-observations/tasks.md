## 1. 人物層目錄與讀檔後端

- [x] 1.1 **Requirement: Person-level report directory layout** — 新增 `backend/src/person_trends.rs`：依 `person_id` 解析 `display_name`，讀取 `reports/_people/{display_name}/` 下 `index.md`、`{YYYY-MM}.md`、`_notes.md`；掃描 `reports/` 時排除 `_people`（design：**人物層目錄佈局**）。驗證：`backend/tests/person_trends.rs` scenario **Person directory is separate from project directories**
- [x] 1.2 **Requirement: Person trends read API** — 實作 `GET /api/people/:id/trends` 掛載於 `server.rs`；純讀檔組 JSON、不寫 SQLite（design：**趨勢 API 讀檔、不寫 DB**）；未知 person 回 404、缺檔回 200 空欄位。驗證：integration test **Trends API returns person-level index content** 與 **Missing person-level files return empty sections**
- [x] 1.3 **Requirement: Loose-format migration support for person observations** — 無 frontmatter 的 `index.md` 全文回傳為 `long_term_observation`（design：**寬鬆遷移僅文件 + 讀檔容錯**）。驗證：**Legacy markdown displays without frontmatter**

## 2. Manifest 與 workflow

- [x] 2.1 **Requirement: Weekly batch manifest includes analysis window and authors** — `RunManifest` 與 `write_weekly_manifest` 新增 `person_report_root` 欄位（指向 `{DATA_ROOT_DIR}/reports/_people`；design：**manifest 擴充 `person_report_root`**）。驗證：`backend/tests/runs_execution.rs` **Manifest includes person report root**
- [x] 2.2 **Requirement: Worker executes reviewer skill subprocess per project** — 更新 `skills/reviewer-batch/WORKFLOW.md` §4：每週產報後維護 `_people/{display_name}/index.md`、`YYYY-MM.md`、`_notes.md`（跨專案綜合敘事；design：**reviewer-batch workflow 變更**）；專案層 `index.md` 標為可選專案脈絡（design：**專案層 `index.md` 降級為可選補充**）。驗證：mock run 後 `reports/_people/{name}/index.md` 存在或更新（**Successful run maintains person-level files via workflow**）
- [x] 2.3 更新 `docs/idea/schema.md` §0 目錄樹與 §6 趨勢資料源改指向 `_people/`（**Requirement: Person-level report directory layout**）。驗證：文件審查路徑與 design **人物層目錄使用 `_people` 前綴** 一致

## 3. 前端趨勢檢視

- [x] 3.1 **Requirement: Person trends API for report reader** — 新增 `fetchPersonTrends(personId)` 與 `PersonTrendsResponse` types。驗證：`npm run build` 通過
- [x] 3.2 **Requirement: Person trends API for report reader** — 人員檢視新增「本週 / 趨勢」切換；趨勢區塊渲染 `long_term_observation`、`growth_timeline`、`historical_pending`；空狀態顯示提示。驗證：手動 smoke **Frontend fetches trends for selected person** 與 **Trends empty state**
- [x] 3.3 **Requirement: Latest weekly report content is served per person** — 確認 `GET /api/people/:id/reports/latest` 不含人物層 `index.md`（長期觀察僅由 trends API 提供）。驗證：**Latest reports excludes long-term observation**

## 4. 遷移文件與 README

- [x] 4.1 **Requirement: Migration documentation for person observations** — 新增 `docs/idea/migration-person-observations.md`：說明 `_people/{display_name}/index.md` 可放自由格式舊筆記、不需轉 `summary.md`（design：**寬鬆遷移文件**）。驗證：**Migration doc references person-level path**
- [x] 4.2 更新 `README.md` 補充人物層目錄與遷移文件連結。驗證：人工審閱 README 含 `_people` 路徑範例

## 5. 端對端驗證

- [x] 5.1 執行 `cargo test -p reviewer-server` 全綠
- [x] 5.2 執行 `cd frontend && npm run build` 通過
