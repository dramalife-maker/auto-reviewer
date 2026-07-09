## Why

產品規格（`docs/idea/spec.md` §0、§2.6）要求以「人」為中心、支援跨專案彙整與跨月長期觀察；但現行檔案佈局把 `index.md`、`YYYY-MM.md`、`_notes.md` 放在 `reports/{專案}/{人}/` 底下，長期觀察實際上**綁在單一專案**，同一人參與多專案時會產生多份互不相通的敘事。使用者亦希望將既有、格式不齊的舊評論遷入系統，作為跨專案人物脈絡的種子資料，而非強迫對齊每週 `summary.md` 契約。

## What Changes

- 新增**人物層**報告目錄：`$DATA_ROOT_DIR/reports/_people/{display_name}/`，承載跨專案長期觀察（`index.md`）、月度軌跡（`YYYY-MM.md`）、歷史待確認（`_notes.md`）。
- 專案層 `reports/{project}/{person}/{date}/` 維持不變，仍負責單專案單期週報（`summary.md` / `report.md`）；專案層 `index.md` 降級為**專案脈絡補充**（可選），不再作為趨勢頁「長期觀察」主資料源。
- 後端新增讀檔 API，依 `person_id` 回傳人物層趨勢內容（長期觀察、成長軌跡、歷史待確認三區塊，對齊 spec §2.6）。
- 前端在人員詳情新增「趨勢」檢視（或次層 Tab），讀取人物層 API；本週總覽仍為跨專案 `summary` 合併。
- 調整 `reviewer-batch` workflow：每週跑完後**同時**維護人物層 `index.md` / `YYYY-MM.md` / `_notes.md`（跨專案綜合敘事），專案層僅保留該專案本週細節。
- 文件化**寬鬆遷移**路徑：舊資料可直接以自由格式 Markdown 放入人物層 `index.md`（或 `_archive/` 子目錄），無需補齊 `mr_count` 等欄位；入庫僅針對符合契約的 `summary.md`。

## Non-Goals

- 自動把不完整舊資料轉成 `summary.md` frontmatter 格式（無 AI 批次轉換工具）。
- 合併或搬移既有專案層 `index.md` 到人物層的自動腳本（僅文件指引，手動或後續 change）。
- 趨勢頁數量圖表、跨人比較、參與度排名。
- 修改 `pending_items` 為跨專案單表（本週待確認仍由 SQLite 驅動；歷史待確認讀 `_notes.md`）。
- 人員合併（兩個 `people` 列合併）。

## Capabilities

### New Capabilities

- `person-trends`：人物層檔案佈局、讀檔 API、前端趨勢檢視、workflow 維護人物層長期檔。

### Modified Capabilities

- `reviewer-execution`：週報 workflow 產出與維護路徑納入人物層 `_people/`；manifest 或 prompt 需帶入人物層 `report_root` 提示。
- `report-reader`：新增趨勢 API 與前端趨勢 Tab；本週 API 行為不變。

## Impact

- Affected specs: `person-trends`（新建）、`reviewer-execution`、`report-reader`
- Affected code:
  - New:
    - `backend/src/person_trends.rs`
    - `backend/tests/person_trends.rs`
    - `docs/idea/migration-person-observations.md`（寬鬆遷移指引）
  - Modified:
    - `backend/src/server.rs`
    - `backend/src/reports.rs`
    - `skills/reviewer-batch/WORKFLOW.md`
    - `frontend/src/app.ts`
    - `frontend/src/api.ts`
    - `frontend/src/types.ts`
    - `frontend/src/style.css`
    - `docs/idea/schema.md`
    - `docs/idea/spec.md`
    - `README.md`
