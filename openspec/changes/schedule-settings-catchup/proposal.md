## Why

週報排程雖已內建 cron，但管理者只能改 MR 輪詢間隔；`enabled`、`weekday`、`run_time`、時區、逾時與並發仍靠 DB／預設值。服務重啟或部署中斷後，規格 §7.3 的「漏跑提示 + 手動補跑」也未落地，排程可信度不足。控制台已有排程卡，應擴成可編輯設定並補上漏跑補償。

## What Changes

- 擴充 `PATCH /api/schedule`：可更新 `enabled`、`weekday`、`run_time`、`tz_offset_min`、`per_project_timeout_sec`、`max_concurrency`、`mr_poll_interval_min`；`cadence` 本輪固定 `weekly`（送其他值 → 400）。
- 控制台排程區改為可編輯表單（週報 + MR）；儲存後提示 cron 需重啟才生效，timeout／concurrency 下一場 run 即生效。
- 啟動／讀取 dashboard 時偵測最近一個已過期的週報 due window；若無覆蓋 run 則回傳 `missed_weekly_run`。
- 新增 `POST /api/schedule/catch-up`：確認後建立與 `manual_all` 同等的批次 run；控制台橫幅提供「立即補跑／稍後」。
- MR 輪詢不做漏跑補償；不做 cron 熱重載、不做 daily／biweekly。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `scheduling`: 排程設定可經 API／控制台完整編輯；週報漏跑偵測與手動 catch-up。

## Impact

- Affected specs: scheduling (modified)
- Affected code:
  - New: (none required as separate crate; helpers in `backend/src/schedule.rs`)
  - Modified: `backend/src/schedule.rs`, `backend/src/dashboard.rs`, `backend/src/server.rs`, `backend/src/runs.rs` (若 catch-up 複用建 run), `backend/tests/scheduling.rs`, `backend/tests/schedule_api.rs`（若存在）, `backend/tests/dashboard.rs`, `frontend/src/app.ts`, `frontend/src/api.ts`, `frontend/src/types.ts`, `frontend/src/style.css`, `docs/idea/schema.md`, `README.md`
  - Removed: (none)

