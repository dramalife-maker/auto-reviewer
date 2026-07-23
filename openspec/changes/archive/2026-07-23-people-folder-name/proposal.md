## Problem

People report 目錄以可變的 `people.display_name` 為鍵（人物層 `reports/_people/{display_name}/`、專案層 `reports/{project}/{display_name}/`）。管理者透過 `PATCH /api/people/{id}` 改名時：

- `rename_person` 只 rename 了人物層 `reports/_people/{old}/`，**專案層** `reports/{project}/{old}/` 資料夾未搬 → 改名後孤立、後續週報寫到新名目錄、舊資料看不到。
- `reports` 表存的絕對路徑 `summary_md_path` / `report_md_path` 指向舊路徑 → 漂移。
- ingest 用 frontmatter `person`（＝舊 display_name）反查 `people.display_name`（＝新名）→ 舊 run 重新 ingest 時反查失效、summary 被跳過。

## Root Cause

系統把「可變的人類顯示標籤（`display_name`）」同時當成「磁碟路徑與 ingest 的穩定鍵」。任何改名都會讓所有以 display_name 組成的路徑與反查同步失效，而現行 rename 只補救了其中一個目錄。

## Proposed Solution

為 `people` 引入不可變的 `folder_name` 當唯一路徑鍵（採方案 A 完整解耦）：

- `folder_name` 建立時＝初始 `display_name`，之後任何 API 都不可改。
- 資料夾名、`summary.md` frontmatter `person`、ingest 反查三處全改用 `folder_name`；`display_name` 退出 headless 契約，只當 API/UI 顯示標籤，可自由改名。
- `rename_person` 改為純 UPDATE `display_name`，**零搬檔**（folder_name 不變 → 所有路徑與 `reports` 存的絕對路徑永不漂移）。
- manifest `authors[]` / `open_pending[]` 帶 `folder_name`；`output-contract.md` 與 `WORKFLOW.md` 契約改以 folder_name 為目錄／person 鍵。

因 `folder_name` 初始＝`display_name`，對從未改名者，產物外觀與現狀完全一致。

## Non-Goals

- 不自動修復「本 migration 前、過去改名已造成的孤立專案層資料夾」。Backfill 一律取現有 `display_name`；既有孤兒屬歷史資料債，不在 migration 內做 filesystem reconciliation。
- 不放寬 `display_name` 唯一性：改名仍拒絕與他人重複的 display_name（HTTP 409），保留現行行為。
- 不改動 API 回應結構：對外仍回 `display_name`，`folder_name` 為後端內部鍵。
- 不改前端。

## Success Criteria

- `people` 具 `folder_name`（UNIQUE NOT NULL），既有列 backfill＝現有 display_name。
- `create_person` 一次性設定 `folder_name = display_name`；此後無 API 可改 `folder_name`。
- `PATCH /api/people/{id}` 改名後：`folder_name` 不變、無任何目錄被搬動、`reports` 存的路徑不變、對該人歷史 summary 重新 ingest 仍能反查到正確 person。
- 週報 manifest `authors[]` 含 `folder_name`；headless 產物目錄與 frontmatter `person` 使用 `folder_name`；ingest 以 `folder_name` 反查 person。
- 既有「從未改名」的資料流測試全綠（folder_name＝display_name 情形零回歸）。

## Impact

- Affected specs: `person-identity`, `people-settings`, `reviewer-execution`
- Affected code:
  - New:
    - backend/migrations/016_people_folder_name.sql
  - Modified:
    - backend/src/identity.rs
    - backend/src/person_trends.rs
    - backend/src/reports.rs
    - backend/src/summary.rs
    - backend/src/runs.rs
    - skills/reviewer-batch/output-contract.md
    - skills/reviewer-batch/WORKFLOW.md
  - Removed: (none)
- Affected tests:
  - backend/tests/identity.rs
  - backend/tests/person_trends.rs
  - backend/tests/report_reader.rs
  - backend/tests/runs_execution.rs
