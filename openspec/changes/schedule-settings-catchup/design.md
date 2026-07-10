## Context

`schedule_config` 單列已驅動週報 cron 與 MR 輪詢；`GET /api/schedule` 已回傳完整欄位，但 `PATCH` 僅接受 `mr_poll_interval_min`。控制台可改 MR 間隔，週報參數唯讀。Worker 每次 run 會讀 `per_project_timeout_sec`／`max_concurrency`；cron 僅在 `start_scheduler` 註冊一次。`docs/idea/spec.md` §7.3 建議重啟時檢查漏跑並提示補跑，尚未實作。

## Goals / Non-Goals

**Goals:**

- 讓管理者在控制台編輯週報與 MR 排程相關欄位並持久化。
- 偵測最近一個錯過的週報 due window，並在控制台提示；確認後手動補跑。
- 清楚區分「需重啟才生效」（cron）與「下一場 run 即生效」（timeout／concurrency）。

**Non-Goals:**

- `cadence` 的 daily／biweekly 實作。
- JobScheduler 熱重載。
- MR 輪詢漏跑補償。
- 自動補跑（無確認）。
- 完整執行歷史頁（屬 D）。
- 多使用者「誰按了補跑」歸屬。

## Decisions

### 控制台內嵌編輯，不另開排程頁

- **選擇**：擴充 dashboard 排程區為表單。
- **理由**：單列設定不值得獨立 IA；與現有 UI 一致。
- **替代**：獨立排程頁 — 導航膨脹。

### Cron 需重啟；timeout／concurrency 即時

- **選擇**：PATCH 成功即寫 DB。UI 提示：影響 cron 的欄位需重啟 `reviewer-server`；`per_project_timeout_sec`／`max_concurrency` 下一場 run 生效（worker 已每次讀 DB）。
- **理由**：熱重載 cron 複雜；與現有 MR 間隔提示一致。
- **替代**：儲存後自動重建 scheduler — 本輪不做。

### 漏跑只看最近一個週報 window

- **選擇**：`enabled=1` 時算最近已過去的 `due_at`；若無 `trigger IN ('schedule','manual_all')` 且 `started_at >= due_at - 6h` 且 `status IN ('success','partial','running','queued')` 的 run → `missed_weekly_run`。不回溯更早週次；`enabled=0` 不提示。
- **理由**：覆蓋部署／短暫中斷；避免一次補很多週的過時報告。
- **替代**：自動補跑或回溯 N 週 — 拒絕。

### Catch-up 專用端點，內部等同 manual_all

- **選擇**：`POST /api/schedule/catch-up` 建立批次 run（與 `manual_all` 相同管線）；衝突 → 409。「稍後」僅 `sessionStorage` 隱藏橫幅。
- **理由**：語意清楚；不永久 dismiss 漏跑狀態。
- **替代**：只叫現有「全部執行」按鈕 — 較難在 API 層表達 catch-up 意圖與測試。

### cadence 本輪鎖定 weekly

- **選擇**：PATCH 若帶 `cadence` 且非 `weekly` → 400；UI 顯示唯讀「每週」。
- **理由**：`build_cron_expression` 僅支援 weekly。

## Implementation Contract

### Schedule update API

- **Behavior**：管理者可更新排程欄位；非法值被拒絕；成功回傳完整設定。
- **Interface**：`PATCH /api/schedule` 可選欄位：`enabled`, `weekday`, `run_time`, `tz_offset_min`, `per_project_timeout_sec`, `max_concurrency`, `mr_poll_interval_min`；可選 `cadence` 僅允許 `weekly`。
- **Failure modes**：校驗失敗 → 400；成功 → 200 + `ScheduleConfigResponse`。
- **Acceptance**：`schedule_api`／`scheduling` 整合測試覆蓋各欄位更新與非法值。
- **In scope**：API、控制台表單、漏跑偵測與 catch-up。
- **Out of scope**：見 Non-Goals。

### Missed weekly run detection

- **Behavior**：dashboard／schedule 讀取時，若最近 due window 未被覆蓋，回傳 `missed_weekly_run: { due_at, label }`，否則 `null`。
- **Interface**：加在 `GET /api/dashboard` 的 `schedule`（或頂層）與／或 `GET /api/schedule`。
- **Acceptance**：單元／整合測試：有覆蓋 run → null；無覆蓋且 enabled → 物件；disabled → null。

### Catch-up

- **Behavior**：確認補跑後建立批次 run；橫幅可稍後隱藏（session）。
- **Interface**：`POST /api/schedule/catch-up` → `202 { run_id }` 或與現有 create-run 對齊的成功形狀；衝突 → 409。
- **Acceptance**：整合測試成功建 run；衝突 409；前端 build 通過。

## Risks / Trade-offs

- [管理者改 cron 後忘記重啟] → UI 明確提示；文件同步。
- [6h 容差誤判] → 文件化；過嚴可調但本輪固定 6h。
- [manual_project 不算覆蓋] → 刻意：單專案跑不代表整批排程已執行。

## Migration Plan

- 無 DB migration。
- 前後端一併部署。
- Rollback：還原二進位；已寫入的 schedule_config 保留。

## Open Questions

（無）

