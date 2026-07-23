## Context

現況：`people.display_name` 同時扮演三個角色——(1) 磁碟目錄名（人物層 `reports/_people/{display_name}/`、專案層 `reports/{project}/{display_name}/`）、(2) `summary.md` frontmatter `person` 欄位、(3) ingest 反查鍵（`summary.rs` 的 `resolve_person_id_by_display_name`）。改名（`PATCH /api/people/{id}` → `rename_person`）只 rename 人物層目錄，未搬專案層目錄、未改 `reports` 表存的絕對路徑、未同步 frontmatter，導致路徑漂移與 ingest 反查失效。

已查證約束：
- `reports` 表（migration 001）以絕對路徑欄 `report_md_path` / `summary_md_path`（NOT NULL）記錄產物位置。
- ingest 於 `summary.rs` 的 `upsert_summary` 用 `resolve_person_id_by_display_name(frontmatter.person)` 反查。
- 路徑組法集中在：`person_trends.rs` 的 `person_trends_dir`（人物層）、`reports.rs` 的 `pending_dir` / `discover_pending_observation_projects`、`summary.rs` 的 `reingest_person_summaries`（以 display_name 掃 `{project}/{display_name}/`）。
- manifest 由 `runs.rs` 的 `write_weekly_manifest` 產生；`identity.rs` 的 `prepare_manifest_authors` 建 `ManifestAuthor{ email, git_name, person_id, display_name }`。
- headless 契約 `skills/reviewer-batch/output-contract.md` 明訂 `person` == `display_name` == 目錄名；`WORKFLOW.md` 以 `authors[].display_name` 當目錄名與 frontmatter person。
- `display_name` 目前於 `create_person` / `rename_person` 有唯一性守衛。

## Goals / Non-Goals

**Goals:**

- 引入不可變 `folder_name` 當唯一路徑鍵，使 display_name 改名不再造成路徑漂移或 ingest 反查失效。
- 對「從未改名」的資料，產物外觀與行為零變化。
- 改名成為零副作用的純 DB 操作（無 filesystem 搬移、無路徑改寫）。

**Non-Goals:**

- 不自動 reconciliation 過去改名遺留的孤立專案層目錄（見 Risks）。
- 不放寬 display_name 唯一性守衛。
- 不改 API 回應結構與前端。
- 不改 ingest 的 pending/resolve 語意，只改 person 反查鍵。

## Decisions

**D1 — person 穩定鍵＝`folder_name`（方案 A1）。**
資料夾名、frontmatter `person`、ingest 反查三處全用 `folder_name`。替代方案：A2 frontmatter 增 `display_name` 欄、A3 ingest 改吃路徑段、A4 frontmatter 用 person_id。選 A1 因單一穩定鍵最不易寫錯，且 folder_name 初始＝display_name 使既有產物外觀不變；A2/A3 為「summary 檔內秀最新 display_name」這一弱需求付雙欄位或解析改動的代價，而該顯示需求 UI 可由 `reports JOIN people` 滿足；A4 犧牲 summary 可讀性與 agent 直覺。

**D2 — schema：`people.folder_name TEXT`（UNIQUE NOT NULL）。**
migration `016_people_folder_name.sql`：新增欄位並 backfill＝現有 `display_name`。因既有 display_name 實質唯一，backfill 不產生 UNIQUE 衝突。

**D3 — `create_person` 一次性設定 folder_name。**
INSERT 同時寫 `display_name` 與 `folder_name`（皆＝trim 後初始名）。此後無任何 API 路徑可改 `folder_name`（immutable）。

**D4 — `rename_person` 只 UPDATE display_name、零搬檔。**
移除 `fs::rename`、`old_dir/new_dir` 計算、目的地衝突（`PeopleDirectoryConflict`）與「rename 失敗回滾 display_name」邏輯——因不再有 filesystem 操作。保留 empty→400、duplicate display_name→409 守衛。

**D5 — ingest 反查改用 folder_name。**
新增 `identity::resolve_person_id_by_folder_name`；`summary.rs` 的 `upsert_summary` 改用之反查 `frontmatter.person`（現在其值＝folder_name）。順帶修好「舊 run frontmatter 在改名後反查失效」的衍生 bug。frontmatter `person` 不匹配任何 `folder_name` 時，維持現行「跳過該 summary、不建新 people 列」。

**D6 — 所有路徑組法改吃 folder_name。**
`person_trends_dir` / `pending_dir` / `discover_pending_observation_projects` / `reingest_person_summaries` 的 person 參數與內部 `SELECT display_name`（`person_trends.rs`、`reports.rs`）改為取 `folder_name` 組路徑；需要人類標籤處另取 `display_name`。

**D7 — manifest 契約增 folder_name。**
`ManifestAuthor` 與 `ManifestOpenPending` 各加 `folder_name`（`prepare_manifest_authors` / `load_open_pending_for_project` 的 SELECT 補 `p.folder_name`）。跨陣列關聯仍用 `person_id`。`authors[]` 保留 `display_name` 供 agent 在 `report.md` 正文當人類稱呼。

**D8 — headless 契約文件同步。**
`output-contract.md`：`person` 語意改為「必須等於 `authors[].folder_name`」。`WORKFLOW.md`：目錄名與 frontmatter `person` 用 `folder_name`；display_name 僅正文稱呼。

**D9 — `reports` 表存的絕對路徑不改寫。**
folder_name 不可變 → 路徑永不漂移，無需 migration 改寫既有 `summary_md_path` / `report_md_path`。此為方案 A 的核心紅利。

## Implementation Contract

**Behavior：**
- `POST /api/people {display_name}`：建立的 `people` 列 `folder_name == display_name`（trim 後）。
- `PATCH /api/people/{id} {display_name}`：成功後 `display_name` 更新、`folder_name` 不變、`reports/_people/` 與 `reports/{project}/` 下**無任何目錄被搬動**、`reports` 表存的路徑不變。改名後對該人歷史 summary 重新 ingest 仍反查到同一 `person_id`。
- 週報執行：manifest `authors[]` 每筆含 `folder_name`；headless 產物寫在 `{report_root}/{folder_name}/{run_date}/`，`summary.md` frontmatter `person == folder_name`；ingest 以 folder_name 反查。

**Interface / data shape：**
- schema：`people.folder_name TEXT NOT NULL UNIQUE`（NULL 不允許；backfill＝display_name）。
- `identity::resolve_person_id_by_folder_name(pool, folder_name) -> Option<i64>`（新增）。
- `ManifestAuthor { email, git_name, person_id, folder_name, display_name }`。
- `ManifestOpenPending { id, person_id, folder_name, display_name, question }`。
- 路徑函式簽名的 person 參數語意由「display_name」變為「folder_name」（`person_trends_dir`、`pending_dir`、`reingest_person_summaries` 等）。

**Failure modes：**
- 改名為空 → 400；改名撞他人 display_name → 409（沿用）。
- frontmatter `person` 不匹配任何 folder_name → 跳過該 summary、不建新 people 列（沿用現行「未知 person」語意，反查鍵由 display_name 換成 folder_name）。
- migration 對既有列 backfill；folder_name NOT NULL UNIQUE 若既有資料有重複 display_name 會失敗——現行守衛保證不重複，故不預期發生。

**Acceptance criteria：**
- `backend/tests/identity.rs`：create 後 `folder_name==display_name`；rename 後 `display_name` 變、`folder_name` 不變、無目錄搬動、`resolve_person_id_by_folder_name` 於改名後仍回原 id。
- `backend/tests/person_trends.rs` / `report_reader.rs`：改名後既有 `reports` 路徑仍可讀、pending/trends 路徑仍指向 folder_name 目錄。
- `backend/tests/runs_execution.rs`：manifest `authors[]` 含 folder_name；ingest 以 folder_name 反查成功。
- 既有「從未改名（folder_name==display_name）」測試全綠。

**Scope boundaries：**
- In scope：migration + identity.rs（create/rename/新反查）+ person_trends.rs + reports.rs + summary.rs（ingest 反查）+ runs.rs（manifest authors/open_pending）+ 2 份 headless 契約文件 + 測試。
- Out of scope：過去孤兒目錄 reconciliation、display_name 唯一性放寬、API 回應結構、前端、`reports` 路徑改寫。

## Risks / Trade-offs

- [本 migration 前、過去改名遺留的孤立專案層目錄不會被自動修復] → 記入 Non-Goal；backfill 取現有 display_name，往後 folder_name 不可變不再產生新孤兒；若需修既有孤兒另以手動維運處理。
- [folder_name 凍在初始名，歷史 summary 檔內看不到最新 display_name] → 可接受；UI 由 `reports JOIN people` 顯示最新 display_name，summary 為機器產物。
- [migration NOT NULL UNIQUE backfill 依賴既有 display_name 無重複] → 由現行 create/rename 唯一性守衛保證；migration 前若人工塞入重複資料需先清理。

## Migration Plan

1. 套用 `016_people_folder_name.sql`：加 `folder_name TEXT`，`UPDATE people SET folder_name = display_name`，再加 UNIQUE / NOT NULL 約束（SQLite 以重建表或 index 方式達成，於 migration 內處理）。
2. 後端向前相容：對從未改名者 folder_name==display_name，所有路徑與 frontmatter 外觀不變。
3. headless 契約文件與後端同批上線（frontmatter person 語意由 display_name 轉 folder_name，兩者對未改名者相等，無斷點）。
4. 無資料回填 `reports` 路徑、無破壞性 schema 移除。
