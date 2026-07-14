## Context

執行紀錄詳情（`GET /api/runs/{id}` + RunsPage「專案結果」）已能顯示專案 state、error、以及 MR run 的 `skip_summary`。週報寫入 `reports`（含 `run_id`）；MR 草稿落在 `runs/{run_id}/projects/{project_id}/drafts/*.md` 並 ingest 進 `mr_reviews`。管理者結束 run 後仍須自行猜該去 MR 收件匣或報告閱讀器。

約束：不新增 migration；不改 inbox／報告閱讀器篩選能力；讀取行為比照 `skip_summary`（僅非 `running` 時計算），以免 2s 輪詢加重負擔。

## Goals / Non-Goals

**Goals:**

- 結束的 run 詳情中，每個專案揭露是否產出 MR 草稿／週報
- UI 以可點擊連結導向既有頁面（`/mr-inbox`、`/reports/{personId}`）
- 任何專案 state 只要有產出即顯示（含 failed／partial）

**Non-Goals:**

- 依 run／專案篩選 MR 收件匣
- 依 run／日期打開「剛好那份」週報（接受連到該人最新報告頁）
- 執行中即時更新 outputs
- 持久化 outputs 欄位或新 DB 表
- 執行紀錄列表頁顯示产出

## Decisions

### Decision: outputs 以讀取時衍生、不落庫

於 `get_run` 組裝回應時衍生 `outputs`，與 `skip_summary` 相同「結束後才算」。

Alternatives: 寫入 `run_projects` JSON 欄位 → 需 migration 且 finalize 時機易漏，拒絕。

### Decision: MR 草稿計數來源為 drafts 目錄

使用既有 `mr_poll_draft_dir` 路徑，計算 `*.md` 檔數量。與 worker ingest 同源，不依 `mr_reviews.created_at` 時間窗猜測。

Alternatives: `draft_md_path LIKE runs/.../drafts/%` 查 DB → 路徑格式／正規化與平台斜線脆弱；目錄計數更直接。

### Decision: 週報來源為 reports.run_id

`SELECT` `reports` JOIN `people`，條件 `run_id` + `project_id`，回傳 `person_id` 與 `display_name`。

### Decision: API 形狀與省略規則

每個專案可選：

```json
"outputs": {
  "mr_drafts": { "count": 2 },
  "weekly_reports": {
    "people": [{ "person_id": 1, "display_name": "Alice Chen" }]
  }
}
```

- run `status == "running"` → 整個 `outputs` 省略／null（所有專案）
- `mr_drafts.count == 0` → `mr_drafts` 為 null
- `weekly_reports.people` 空 → `weekly_reports` 為 null
- 兩者皆無 → `outputs` 為 null／省略

### Decision: UI 文案與連結

- MR：`已產出 N 份 MR 草稿` + 連結「MR 收件匣」→ `/mr-inbox`
- 週報：`已產出` + 每人名可點 → `/reports/{personId}` + `的週報`
- 人名超過 8：顯示前 8，其餘「…等共 N 人」
- 視覺：次要資訊區（非 danger 紅底），可與 skip 摘要同卡並存

### Decision: 測試落點

- 後端：擴充 `backend/tests/runs_execution.rs`（對齊既有 skip_summary 測試風格）
- 前端：新增 RunsPage 產出提示的單元／元件測試（現無 RunsPage 測試檔則新建）

## Implementation Contract

**Behavior**

- 管理者開啟已結束 run 的詳情：若該專案在此次 run 產出 MR 草稿或週報，專案卡顯示文字提示與可點擊導向
- running 中的詳情／輪詢回應不含 `outputs`
- failed／timeout 專案只要有產出同樣顯示

**Interface / data shape**

- `GET /api/runs/{id}` 專案物件新增可選 `outputs`（上列 JSON）
- 前端 `RunProjectStatus` 對應同形狀選用欄位

**Failure modes**

- drafts 目錄缺失／不可讀 → MR 計數視為 0，不讓整份 run detail 失敗
- reports 查詢失敗 → 走既有 API 錯誤路徑（與其他 DB 讀取一致）
- 無產出 → 省略提示列，不是錯誤

**Acceptance criteria**

- 整合測試：結束的 MR run 在 drafts 放入 N 個 `.md` → `outputs.mr_drafts.count == N`，且含可序列化連結所需資料
- 整合測試：結束的週報 run 插入 `reports` 列綁 `run_id` → `outputs.weekly_reports.people` 含 display_name／person_id
- 整合測試：`running` MR／週報 run → `outputs` 省略或 null
- 前端測試：有 `outputs` 時渲染文案與連結 href；無 `outputs` 時不渲染提示列

**Scope boundaries**

- In：run detail API + RunsPage 專案結果卡 + run-history 規格／測試
- Out：inbox／reports 深篩、執行中即時 outputs、DB schema 變更、列表頁

## Risks / Trade-offs

- [Risk] MR 連結進全站收件匣，同專案草稿可能混在其他專案之間 → 接受；文案標明數量
- [Risk] 週報連結開到「最新」而非本次日期 → 接受；本階段不深連結
- [Risk] drafts 目錄有檔但 ingest 失敗，UI 仍顯示計數 → 文件化為「執行產物存在」語意；不改成本期 ingest 修復
