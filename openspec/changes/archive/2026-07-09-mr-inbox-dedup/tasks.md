## 1. 收件匣閘門核心（mr_reviews）

- [x] 1.1 實作 `load_inbox_blocked_rounds`（比對鍵與擋住狀態：`(project_id, mr_iid, review_round)` + `draft`/`ignored`）：對指定 `project_id` 回傳 blocked 集合；驗證：`cargo test` seed SQLite 後斷言集合內容。
- [x] 1.2 實作 `filter_eligible_by_inbox`：`force=false` 時跳過命中集合的 eligible 項目並附 `skip_reason`（`inbox_draft` / `inbox_ignored`），`force=true` 時原樣輸出；驗證：單元測試涵蓋 draft、ignored、published 不擋、force 繞過（對應 Requirement: MR scan worker skips inbox-blocked review rounds、比對鍵與擋住狀態）。

## 2. Worker 串接（triage 之後、spawn 之前）

- [x] 2.1 Worker 收件匣閘門位置（triage 之後、spawn 之前）：在 `process_mr_run_project` 讀取 `eligible_mrs.json` 後呼叫閘門過濾，僅對 `to_run` spawn agent；跳過項寫 warn 日誌；驗證：整合測試 seed draft 後斷言 spawn 次數為 0。
- [x] 2.2 將 run 的 `force` 旗標從 API 傳遞至 worker（run 或 run_projects metadata），排程 `mr_poll` 永遠 `force=false`；驗證：整合測試 `manual_mr_poll` + `force=true` 時仍 spawn（對應 Decisions: force 繞過閘門）。

## 3. API 與發佈標記

- [x] 3.1 `POST /api/projects/:id/mr-scan` 解析 query `force`（`1`/`true` 為真），建立 run 時持久化旗標；驗證：HTTP 測試或 handler 單元測試斷言 run 記錄帶 force（對應 Requirement: Manual MR scan supports force bypass of inbox gate）。
- [x] 3.2 `mr_reviews::publish` 張貼前合併 `By: AI Agent` footer（已含則不重複），`published_body` 與實際 note 一致；驗證：單元測試 mock `glab` 或檢查組裝字串（對應 Requirement: Publishing appends GitLab dedup marker to posted note、Decisions: publish 附加去重標記）。

## 4. 前端

- [x] 4.1 專案設定或 MR 掃描入口新增「強制重掃」，呼叫 `POST /api/projects/:id/mr-scan?force=1`；驗證：手動點擊後 network 請求含 `force=1` 且後端接受 202。

## 5. 驗證與收尾

- [x] 5.1 執行 `cargo test -p reviewer-server`（必要時 `CARGO_TARGET_DIR=target-test`）全綠；驗證：CI 等價測試通過。
- [x] 5.2 執行 `spectra validate mr-inbox-dedup` 通過；驗證：CLI exit 0。
