## 1. Schedule update API

- [x] 1.1 擴充 `PATCH /api/schedule`（Schedule configuration can be updated via API）：支援 `enabled`／`weekday`／`run_time`／`tz_offset_min`／`per_project_timeout_sec`／`max_concurrency`／`mr_poll_interval_min`；`cadence` 僅允許 `weekly`；非法值 400。對齊 design「cadence 本輪鎖定 weekly」。驗證：整合測試覆蓋成功更新、非 weekly 400、timeout 0 → 400。

## 2. Missed weekly detection and catch-up

- [x] 2.1 實作 Missed weekly run detection／Missed weekly schedule is detected for catch-up：計算最近 due_at；6h 容差內 `schedule`／`manual_all` 且狀態 success／partial／running／queued 視為已覆蓋；`enabled=0` 回 null。對齊 design「漏跑只看最近一個週報 window」「Missed weekly run detection」。驗證：單元或整合測試覆蓋 missed／covered／disabled 三條路徑。
- [x] 2.2 實作 Operator can confirm weekly catch-up run：`POST /api/schedule/catch-up` 建立等同 `manual_all` 的批次 run；衝突 409。對齊 design「Catch-up 專用端點，內部等同 manual_all」。驗證：整合測試成功回 run_id、衝突 409。

## 3. Dashboard schedule UI

- [x] 3.1 控制台排程區改為可編輯表單（Dashboard schedule panel edits schedule settings；控制台內嵌編輯，不另開排程頁）；cadence 唯讀 weekly；儲存後提示 cron 需重啟、timeout／concurrency 下一場生效；有 `missed_weekly_run` 時顯示補跑橫幅（立即補跑／sessionStorage 稍後）。對齊 design「控制台內嵌編輯，不另開排程頁」「Cron 需重啟；timeout／concurrency 即時」。驗證：`npm run build`；手動確認儲存與橫幅流程。

## 4. Docs

- [x] 4.1 更新 `README.md` 與 `docs/idea/schema.md`：記載可 PATCH 欄位、重啟生效範圍、漏跑判定與 catch-up 端點。驗證：文件內容審查含上述要點。


