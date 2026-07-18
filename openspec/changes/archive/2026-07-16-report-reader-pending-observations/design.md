## Context

軌道 2（MR 掃描）會把工程師觀察片段寫入 `reports/<project>/<person>/_pending/mr-{iid}-round-{round}.md`。僅當對應 `mr_reviews.status='published'` 時，週報 batch 才折入 `summary.md` 並刪除檔案；`draft`／`ignored`／已 published 但尚未跑週報的片段都仍留在 `_pending/`。

報告閱讀器目前只暴露週報摘要與 SQLite `pending_items`（待確認），管理者在 1on1 前看不到這些「尚未進週報」的觀察。

## Goals / Non-Goals

**Goals:**

- 在最新週報 API 與閱讀器 UI 暴露每位人員、各專案 `_pending/` 內仍存在的觀察片段全文
- 標示對應 MR review 狀態（draft／published／ignored／unknown），讓管理者一眼分辨能否期待下週折入
- 與既有「待確認」區塊語意分離，避免混淆兩種 pending

**Non-Goals:**

- 不在閱讀器內發佈／忽略／編輯 MR 草稿（仍走收件匣）
- 不改變週報消費 `_pending/` 的規則（仍僅 published 可折入）
- 不解析觀察片段 heading 結構；正文以整檔 markdown 回傳
- 不把片段計入 sidebar `open_pending_count`（該計數仍只指 SQLite 待確認）
- 不回溯已從 `_pending/` 刪除（已折入週報）的片段

## Decisions

### 資料來源：掃描檔案系統 `_pending/`，再 join `mr_reviews`

以「檔案還在」為真實來源（與週報消費契約一致）。對每個 `mr-{iid}-round-{round}.md` 查 `mr_reviews` 取 `status`；無列則 `status=unknown`。不採用「只查 published 列」——那會漏掉 draft／ignored。

替代方案：只回傳 `load_published_pending_snippets` 結果 → 拒絕，因為管理者更常需要看尚未發佈的觀察。

### API：擴充既有 `GET /api/people/:id/reports/latest`

在每個 `LatestReportItem` 加 `pending_observations: PendingObservation[]`，避免第二次 round-trip。即使該專案本週無週報列，只要該人在專案有 `_pending/` 檔，仍應出現在回應中——實作上：以最新 `report_date` 的專案卡片為主掛載；若某人某專案有 `_pending/` 但無該日報告，則在同一次回應額外附上僅含 `project_name` + `pending_observations` 的卡片（`id`／摘要欄位可為空或省略策略見 Contract）。

簡化決策（已撤銷）：~~只掛在已有最新週報的專案卡片上~~。**現行為**：有最新週報的專案照常掛載；若某人某專案有 `_pending/`（或剩餘 open `pending_items`）但無該日報告／尚無任何週報，則追加僅含觀察／待確認的合成專案卡（`id` 為負的 `project_id`，`is_read=true`，摘要欄位為空）。人物存在但無週報、亦無 pending → 回傳 `report_date: null` 與空 `projects`（非 404）。人物不存在 → 仍 404。

`report_date` 型別改為可選（`string | null`），無週報時為 `null`。

### 片段欄位形狀

```json
{
  "mr_iid": 4,
  "review_round": 1,
  "mr_title": "optional from mr_reviews",
  "status": "draft",
  "filename": "mr-4-round-1.md",
  "content": "<full markdown body>"
}
```

`status` ∈ `draft` | `published` | `ignored` | `unknown`。排序：`published` 優先（即將折入），其後 `draft`，再 `ignored`，最後 `unknown`；同 status 以 `mr_iid`、`review_round` 升序。

### UI：總覽彙整 + 專案 tab 詳文

- 總覽：獨立區塊「待折入觀察」，依專案分組；每則顯示 status pill、MR 標題／iid、可展開或直接顯示全文（首版直接顯示全文，與趨勢頁 raw md 風格一致）
- 專案 tab：同區塊置於「待確認」上方或下方——放在**待確認上方**（觀察是脈絡，待確認是要勾的行動項）
- 不提供 resolve／publish 按鈕

### 檔名解析失敗

不符合 `mr-{iid}-round-{round}.md` 的檔略過並 warn log；不讓整次 API 失敗。讀檔 IO 錯誤同處理（略過該檔 + warn）。

## Implementation Contract

**Behavior**

- 呼叫 `GET /api/people/:id/reports/latest` 時：人物存在則 200；每個有最新週報的專案物件包含 `pending_observations`；另對「有 `_pending/` 檔或 open pending_items、但無該日週報」的專案回傳合成卡
- `report_date` 可為 `null`（尚無週報）
- 前端報告閱讀器在總覽與專案 tab 渲染該陣列；空陣列不顯示區塊；僅有合成卡時仍顯示「待折入觀察」而非整頁「尚無週報」

**Interface / data shape**

- `PendingObservation`: `{ mr_iid: number, review_round: number, mr_title: string | null, status: "draft"|"published"|"ignored"|"unknown", filename: string, content: string }`
- `LatestReportItem.pending_observations: PendingObservation[]`（必填，預設 `[]`）

**Failure modes**

- 人物不存在 → 既有 404
- 無最新週報 → 既有空／null 行為不變
- 單一片段讀取／解析失敗 → 略過該檔，其餘照常回傳

**Acceptance criteria**

- 整合測試：在測試 data root 放入 draft 與 published 各一檔，API 回傳兩者且 status 正確
- 整合測試：檔案已刪（已消費）則不出現
- 前端：有片段時總覽出現「待折入觀察」；無則不出現該區塊

**Scope boundaries**

- In：latest API + ReportsPage 顯示
- Out：sidebar badge、收件匣連動、週報消費邏輯、trends API

## Risks / Trade-offs

- [Risk] 大片段全文塞進 latest 回應可能偏大 → Mitigation：首版接受；若超過實務上限再加摘要／懶載
- [Risk] display_name 與資料夾名不一致導致找不到 `_pending/` → Mitigation：沿用既有 person `display_name` 慣例（與 trends／週報相同）；無目錄則空陣列
- [Risk] `unknown` status 片段可能是孤兒檔 → Mitigation：仍顯示並標 unknown，方便管理者發現
