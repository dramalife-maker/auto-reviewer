## 1. Schema

- [x] 1.1 新增 migration `backend/migrations/016_people_folder_name.sql`：`people` 加 `folder_name TEXT`，`UPDATE people SET folder_name = display_name`，再施加 NOT NULL + UNIQUE 約束（SQLite 以重建表或唯一索引達成）（實作 Requirement: People have an immutable folder_name path key 的 schema 與 backfill 部分）。行為：既有列 `folder_name` 等於現有 `display_name`。驗證：`backend/tests/identity.rs` 斷言既有 person backfill 後 `folder_name==display_name`；migration 套用無錯。

## 2. 建立與改名（identity.rs）

- [x] 2.1 修改 `backend/src/identity.rs` 的 `create_person`：INSERT 同時寫入 `display_name` 與 `folder_name`（皆為 trim 後初始名）（完成 Requirement: Create person API 的 folder_name 設定）。行為：新建 person 的 `folder_name` 等於初始 `display_name` 且此後不由任何 API 改動。驗證：`backend/tests/identity.rs` 新增測試斷言 create 後 `folder_name==display_name`。
- [x] 2.2 修改 `backend/src/identity.rs` 的 `rename_person`：改為僅 UPDATE `display_name`，移除 `fs::rename`、`old_dir`/`new_dir` 計算、`PeopleDirectoryConflict` 與 rename 失敗回滾邏輯；保留 empty→400、duplicate display_name→409 守衛（實作 Requirement: Person display name can be renamed 的零搬檔語意）。行為：改名後 `display_name` 變、`folder_name` 不變、無任何目錄被搬動、`reports` 存的路徑不變。驗證：`backend/tests/identity.rs` 斷言改名後 folder_name 不變且目錄未被 rename。

## 3. Ingest 反查改用 folder_name（summary.rs / identity.rs）

- [x] 3.1 在 `backend/src/identity.rs` 新增 `resolve_person_id_by_folder_name(pool, folder_name) -> Option<i64>`，並將 `backend/src/summary.rs` 的 `upsert_summary` 改用之反查 `frontmatter.person`；不匹配時維持「跳過該 summary、不建新 people 列」（實作 Requirement: Summary files are parsed into reports and pending items 的 folder_name 反查）。行為：frontmatter `person`（＝folder_name）反查 person，改名後仍解析到同一 person。驗證：`backend/tests/runs_execution.rs` 或 `report_reader.rs` 斷言改名後以 folder_name 反查 ingest 成功、未知 folder_name 被跳過。

## 4. 路徑組法改吃 folder_name（person_trends.rs / reports.rs / summary.rs）

- [x] 4.1 將 `backend/src/person_trends.rs` 的 `person_trends_dir` 與其取值處（`SELECT display_name`）、`backend/src/reports.rs` 的 `pending_dir` / `discover_pending_observation_projects`、`backend/src/summary.rs` 的 `reingest_person_summaries` 的 person 路徑鍵改為 `folder_name`（`SELECT folder_name` 組路徑），需要人類標籤處另取 `display_name`（完成 Requirement: People have an immutable folder_name path key 的路徑鍵一致性）。行為：人物層與專案層所有目錄以 folder_name 定位，改名不影響。驗證：`backend/tests/person_trends.rs` / `report_reader.rs` 斷言改名後 trends/pending 路徑仍指向 folder_name 目錄且可讀。

## 5. Manifest 契約增 folder_name（runs.rs / identity.rs）

- [x] 5.1 為 `backend/src/identity.rs` 的 `ManifestAuthor` 與 `backend/src/runs.rs` 的 `ManifestOpenPending` 各加 `folder_name` 欄，`prepare_manifest_authors` 與 `load_open_pending_for_project` 的 SELECT 補 `p.folder_name`；`authors[]` 保留 `display_name`（實作 Requirement: Weekly manifest includes resolved authors 的 folder_name 欄）。行為：manifest `authors[]` 每筆含 `folder_name` 與 `display_name`，跨陣列關聯用 `person_id`。驗證：`backend/tests/runs_execution.rs` 斷言 manifest `authors[]` 含 folder_name。

## 6. Headless 契約文件

- [x] 6.1 更新 `skills/reviewer-batch/output-contract.md`（`person` 語意改為必須等於 `authors[].folder_name`）與 `skills/reviewer-batch/WORKFLOW.md`（目錄名與 frontmatter `person` 用 `folder_name`；display_name 僅正文稱呼）（實作 Requirement: Reviewer-batch workflow uses manifest authors 的 folder_name 目錄鍵）。行為：headless agent 依 folder_name 建目錄與寫 frontmatter person。驗證：文件中 `person` / 目錄規則描述引用 `folder_name`；與 spec delta 一致。

## 7. 驗證與回歸

- [x] 7.1 執行 `cargo test`（backend）確認新測試（1.1、2.1、2.2、3.1、4.1、5.1）全綠，且既有「從未改名（folder_name==display_name）」的 identity / person_trends / report_reader / runs_execution 測試全綠。行為：對未改名資料零回歸、改名情境路徑與反查全穩。驗證：測試全數通過。
