## Context

`GET /api/runs/{id}` 已回傳 trigger／status／各專案 `state`／`error`，供前端 polling。沒有列表 API。MR 掃描的 skip 原因寫在 `$DATA_ROOT_DIR/runs/{run_id}/projects/{project_id}/eligible_mrs.json` 的 `skipped[]`（triage + inbox-gate），僅 log／檔案，Web 不可見。控制台只有 `last_run` 摘要。

## Goals / Non-Goals

**Goals:**

- 可瀏覽歷史 runs，並點進單次明細（各專案狀態、耗時、錯誤）。
- MR 類 run 可看到 skip 原因摘要（依 reason 計數 + 明細列表）。
- 控制台最近執行入口 + 獨立執行紀錄視圖（V1）。

**Non-Goals:**

- 即時 stdout／log 串流、agent transcript 全文。
- 刪除／封存 runs、自動清理政策。
- 新 skip DB 表或把每次 skip 寫入 SQLite。
- 產量圖表、跨 run 統計儀表。
- 認證與操作者歸屬。

## Decisions

### 控制台入口 + AppView `runs`，側欄可進

- **選擇**：`AppView = 'runs'`；側欄「執行紀錄」；控制台「最近執行」顯示最近 5 筆並連到完整列表／明細。
- **理由**：V1；歷史查詢需要獨立視圖，側欄避免只能從控制台鑽入。
- **替代**：僅控制台展開 — 查歷史差。

### Skip 摘要讀 `eligible_mrs.json`，不建表

- **選擇**：明細 API 對 MR trigger（`mr_poll`／`manual_mr_poll`）讀各專案 `eligible_mrs.json`，彙總 `skipped[]` 為 `skip_summary: { by_reason: { reason: count }, items: [{ mr_iid, skip_reason }] }`（items 上限 100）。檔案缺失 → 空摘要，不 500。
- **理由**：檔案已是真相來源；避免雙寫。
- **替代**：migration 新表 — 本輪過重。

### 列表分頁與篩選從簡

- **選擇**：`GET /api/runs?limit=50&offset=0`；可選 `trigger`、`status` query。`limit` 預設 50、最大 200。
- **理由**：夠用；不做游標分頁。
- **替代**：無限捲動 + cursor — 不必要。

### 明細擴充既有 GET，不另開路徑

- **選擇**：擴充 `GET /api/runs/{id}`：加入 `duration_sec`、`note`；每個 project 加 `duration_sec`／`started_at`／`finished_at`（既有欄位）；MR run 加 per-project 或 run-level `skip_summary`（採 **per-project** `skip_summary` 掛在各 project 物件上，較好對應多專案 MR poll）。
- **理由**：前端已用此端點 polling；擴充相容。
- **替代**：`/api/runs/{id}/skips` — 多一次往返。

### Dashboard 最近 5 筆

- **選擇**：`GET /api/dashboard` 增加 `recent_runs: RunListItem[]`（最多 5），或前端打 `GET /api/runs?limit=5`。採 **dashboard 內嵌 recent_runs** 減少首屏請求。
- **理由**：控制台已載入 dashboard。
- **替代**：前端另打 list — 也可，但多一 round-trip。

## Implementation Contract

### Runs list API

- **Behavior**：可列出歷史執行，依 `started_at` 降序。
- **Interface**：`GET /api/runs` → `{ runs: [...], total: number }`；每筆含 `id`, `trigger`, `status`, `started_at`, `finished_at`, `duration_sec`, `project_total`, `project_skipped`。
- **Failure modes**：非法 limit → 400。
- **Acceptance**：整合測試插入多筆後斷言順序與分頁。
- **In scope**：list、detail 擴充、skip 摘要、前端視圖與控制台入口。
- **Out of scope**：見 Non-Goals。

### Run detail with skip summary

- **Behavior**：點進 run 可見各專案結果；MR run 可見為何跳過。
- **Interface**：`GET /api/runs/{id}` 擴充如上；未知 id → 404。
- **Acceptance**：MR fixture 寫入 `eligible_mrs.json` 後 API 回傳對應 `by_reason`／items。

### Runs UI

- **Behavior**：側欄與控制台可進入列表；點 run 看明細；失敗／timeout 高亮；skip 按原因分組顯示。
- **Acceptance**：`npm run build`；手動確認列表與明細。

## Risks / Trade-offs

- [舊 MR run 無 eligible 檔] → 空 skip_summary，可接受。
- [items 截斷 100] → by_reason 仍完整計數；文件註明。
- [大 limit 掃表] → max 200；有 `idx_runs_started`。

## Migration Plan

- 無 DB migration。
- 前後端一併部署。
- Rollback：還原二進位。

## Open Questions

（無）

