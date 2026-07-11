## Why

`pending_items` 已從週報 ingestion 寫入，並驅動左欄與控制台的待確認計數，但管理者無法在 Web 上將項目標為已釐清。規格強調的「跨月閉環」因此斷在半路：本週只能看、趨勢 `_notes.md` 也不會反映人工決議。現在雙軌與閱讀器已可用，應先補上閉環操作，讓 1on1 追蹤真正可結束。

## What Changes

- 新增待確認閉環能力：`PATCH /api/pending-items/:id`（`open → resolved`），以及列出某人待確認的查詢 API。
- **BREAKING**：`GET /api/people/:id/reports/latest` 各專案卡的 `pending: string[]` 改為 `pending_items` 物件陣列（含 `id`，供 checkbox 綁定）；本週 UI 以 DB open 列為準，不再用 `summary.md` 的 `## 待確認` 渲染。
- 閉環成功後同步改寫 `reports/_people/{display_name}/_notes.md`（B1 格式：`- [raised→resolved] ✓ question`）。
- 趨勢「歷史待確認」parser／API 改為結構化（open / resolved），前端以樣式區分。
- 週報 ingestion 對同 person+project+question 且仍 open 的列去重，避免重複 INSERT。
- 本週報告卡提供 checkbox：勾選即閉環，成功後項消失並刷新左欄／控制台計數。

## Capabilities

### New Capabilities

- `pending-closure`: 待確認閉環 API、DB 狀態轉換、`_notes.md` 同步寫入、ingestion 去重，以及本週閱讀器 checkbox 互動。

### Modified Capabilities

- `report-reader`: 最新週報 API 的待確認欄位改為 DB 驅動的 `pending_items`；前端本週卡改用 checkbox 閉環。
- `person-trends`: `_notes.md` 行格式擴充 resolved 形態；趨勢 API／UI 區分 open 與已改善項。

## Impact

- Affected specs: pending-closure (new), report-reader (modified), person-trends (modified)
- Affected code:
  - New: `backend/src/pending_items.rs`, `backend/tests/pending_items.rs`
  - Modified: `backend/src/server.rs`, `backend/src/reports.rs`, `backend/src/summary.rs`, `backend/src/person_trends.rs`, `backend/src/lib.rs`, `frontend/src/api.ts`, `frontend/src/types.ts`, `frontend/src/app.ts`, `frontend/src/style.css`, `docs/idea/schema.md`, `README.md`
  - Removed: (none)
