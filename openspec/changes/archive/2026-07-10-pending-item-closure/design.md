## Context

週報 ingestion 已將 `summary.md` 的 `## 待確認` 寫入 SQLite `pending_items`（預設 `status='open'`），左欄與控制台用 open 計數。本週閱讀器卻仍從 `summary.md` 解析字串列表渲染，沒有 `pending_item.id`，也沒有閉環 API。趨勢 Tab 的「歷史待確認」讀 `reports/_people/{display_name}/_notes.md`（`- [YYYY-MM] question`），與 DB 狀態不同步。規格（`docs/idea/spec.md` §2.5–2.6、`schema.md` §3.2）要求管理者可在 Web 將項目標為 `resolved` 並可選寫回 `_notes.md`。

## Goals / Non-Goals

**Goals:**

- 提供 `open → resolved` 閉環 API，並在本週報告卡以 checkbox 操作。
- 本週待確認改以 DB open 列為準（含穩定 `id`）。
- 閉環後同步改寫人物層 `_notes.md`（B1 格式），趨勢 API／UI 能區分 open 與 resolved。
- 週報 ingestion 對同 person+project+question 的 open 列去重。

**Non-Goals:**

- 人員設定頁、排程 UI、漏跑補償、執行紀錄可觀測性。
- 待確認重開（`resolved → open`）、編輯 question 文字、刪除列。
- 多使用者認證與「誰閉環」歸屬欄位。
- 將趨勢主資料源改為 SQLite；不修改已寫入的 `summary.md` 內容。
- Force rescan、MR 收件匣相關改動。

## Decisions

### DB-first 閉環，檔案為衍生視圖

- **選擇**：`PATCH` 先更新 `pending_items`（`status`、`resolved_date`、`resolution_note`），成功後再改寫 `_notes.md`。檔案寫入失敗時回 HTTP 502，但 DB 維持 `resolved`；回應錯誤訊息說明趨勢檔未同步。
- **理由**：本週閉環與左欄計數是主路徑；`_notes.md` 是趨勢衍生視圖，不應因檔案 I/O 擋住閉環。
- **替代**：檔案與 DB 同成敗（先寫檔再 DB）— 檔案失敗會擋住閉環；雙寫交易過重，不採用。

### `_notes.md` B1 行格式與匹配規則

- **選擇**：
  - open：`- [YYYY-MM] {question}`
  - resolved：`- [YYYY-MM→YYYY-MM] ✓ {question}`；若有 `resolution_note`，行尾加 ` — {note}`
  - 閉環時找第一筆文字完全相符的 open 行（月份括號後的 question 與 DB `question` 全等）並改寫；找不到則 append 一筆 resolved 行；檔案／目錄不存在則建立後寫入。
- **理由**：與現有 `- [` 前綴相容；`→` + `✓` 可機器解析狀態。
- **替代**：checkbox `- [ ]`/`- [x]` — 與現有 `- [YYYY-MM]` 衝突；刪行另存 resolved 檔 — 失去歷史軌跡。

### 本週 API 以 `pending_items` 取代 summary 字串

- **選擇**：**BREAKING** — `LatestReportItem.pending: string[]` 改為 `pending_items: PendingItem[]`（至少含 `id`, `question`, `status`, `raised_date`, `project_id`, `project_name`）。每張專案卡只含該專案 `status='open'` 的列。`summary.md` 的 `## 待確認` 仍由 workflow 寫入並供 ingestion，前端不再用來渲染 checkbox。
- **理由**：需要穩定 id 才能 PATCH；字串 fuzzy match 脆弱。
- **替代**：保留 `pending` 字串並另加 id 對照 — 雙軌易漂移。

### resolved_date 使用排程時區當月

- **選擇**：`resolved_date` 為 `YYYY-MM`，依 `schedule_config.tz_offset_min`（預設 480）計算「現在」所在月份。
- **理由**：與週報 `raised_date`、排程語意一致，避免 UTC 跨日切錯月。
- **替代**：純 UTC `datetime('now')` 切月 — 台北深夜易錯月。

### Ingestion 去重僅針對仍 open 的同文問題

- **選擇**：INSERT 前若存在同 `person_id` + `project_id` + `question` 且 `status='open'` 則 skip；已 `resolved` 的同文可再 INSERT 為新 open 列。
- **理由**：避免每週重複堆疊同一未閉環問題；允許議題再次浮現。
- **替代**：全域 unique(question) — 過嚴，擋住合理重提。

### 趨勢 historical_pending 結構化

- **選擇**：`historical_pending` 由 `string[]` 改為物件陣列：`question`, `status` (`open`|`resolved`), `raised_month`, `resolved_month`（optional）, `resolution_note`（optional）, `raw_line`。前端 open 用 warning、resolved 用 success + ✓。趨勢 Tab 不做閉環操作。
- **理由**：UI 需狀態；仍以檔案為趨勢主資料源。
- **替代**：繼續回傳原始字串、前端 regex — 契約不清。

## Implementation Contract

### Closure API

- **Behavior**：管理者可將一筆 open 待確認標為 resolved；本週卡該項消失；左欄 `open_pending_count` 與控制台 `pending_count` 下降；人物層 `_notes.md` 出現或更新對應 resolved 行。
- **Interface**：
  - `GET /api/people/{id}/pending-items?status=open|resolved|all`（預設 `open`）→ 陣列，欄位含 `id`, `person_id`, `project_id`, `project_name`, `report_id`, `question`, `status`, `raised_date`, `resolved_date`, `resolution_note`
  - `PATCH /api/pending-items/{id}` body：`{ "status": "resolved", "resolution_note"?: string }` → 200 回傳更新後物件
- **Failure modes**：
  - 未知 id → 404
  - 已 resolved 再 PATCH → 409
  - body `status` 非 `resolved` → 400
  - DB 更新成功但 `_notes.md` 寫入失敗 → 502（DB 保持 resolved）
- **Acceptance**：整合測試覆蓋 open→resolved、409、去重 ingestion、`_notes.md` 改寫／append／建檔；前端勾選後項消失且 badge 更新。
- **In scope**：上述 API、latest reports 欄位變更、ingestion 去重、trends 結構化、本週 checkbox UI。
- **Out of scope**：見 Non-Goals。

### Latest reports pending shape

- **Behavior**：`GET /api/people/{id}/reports/latest` 各專案卡以 DB open `pending_items` 驅動待確認區塊。
- **Interface**：`pending_items` 陣列取代 `pending` 字串陣列；每元素至少含 `id` 與 `question`。
- **Acceptance**：既有 `report_reader` 整合測試改為斷言新欄位；前端 TypeScript 類型同步。

### Notes file sync

- **Behavior**：閉環後 `_notes.md` 使用 B1 格式；parser 能區分 open／resolved。
- **Interface**：行格式見 Decisions；trends 回應 `historical_pending` 為結構化陣列。
- **Acceptance**：單元／整合測試餵入 open／resolved 行並斷言解析結果；閉環後讀檔可見 `→` 與 `✓`。

## Risks / Trade-offs

- [DB resolved 但 `_notes.md` 失敗] → 回 502 並記錄錯誤；下次 trends 可能仍顯示 open 行；可手動改檔或之後加 repair 工具（本 change 不做）。
- [BREAKING API 欄位更名] → 僅本 repo 前端消費；同步改 `frontend/src/types.ts` 與 `app.ts`。
- [question 文字微調導致 notes 對不上] → append 新 resolved 行，不阻斷閉環；可能留下舊 open 行（可接受，本輪不自動刪）。
- [同文 resolved 後再提] → 新 open 列 + notes 可能多行；符合「議題可再浮現」。

## Migration Plan

- 無需 DB migration（`pending_items` 既有欄位已足夠）。
- 部署後前端必須與後端同時更新（breaking JSON）。
- 既有 `_notes.md` open 行無需預先轉換；僅新閉環寫入 B1 resolved 形態。
- Rollback：還原前後端二進位；已 resolved 的 DB 列與已改寫的 notes 行保留（不自動回滾）。

## Open Questions

（無 — 決策已在 propose 前與使用者確認。）

