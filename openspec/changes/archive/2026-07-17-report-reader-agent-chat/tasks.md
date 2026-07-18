## 1. Schema 與儲存

- [x] 1.1 依「儲存：`person_report_chats` + `person_report_chat_messages`」新增 migration，使 Persist person report Agent Chat turns 可落地；驗證：空庫／既有庫 migration 成功。

## 2. Backend API 與 executor

- [x] 2.1 擴充 executor：支援「Session：每人一條；首次 turn 開新 session」（無 session 不帶 `--resume`；有則 resume），並依「Agent 工作目錄與可寫範圍」組 prompt／add-dir。驗證：單元測試涵蓋有／無 resume 的 command args。
- [x] 2.2 實作「Report chat API returns transcript and accepts agent turns」：GET／POST handlers、成功寫訊息、失敗不寫、404／400／502。驗證：整合測試覆蓋首次 turn、resume、失敗不持久化、GET 空歷史。
- [x] 2.3 依「改檔後 ingest（person-scoped summary reingest）」在成功 agent-turn 後重掃該人 `summary.md` 同步 DB（保留既有 `run_id`）；ingest 失敗不改 200。滿足 Scenario「Successful turn reingests edited summary into DB」。驗證：整合測試改 `one_line`／待確認後 DB 相符且 `run_id` 不變。

## 3. Frontend

- [x] 3.1 依「UI：ReportsPage 右側／底部 Chat 面板」與 Requirement: Report reader hosts person Agent Chat，在 ReportsPage 掛載面板、hydrate、送 turn、成功後 reload latest reports。驗證：前端型別／建置通過；關鍵行為有測試或手動檢查清單。

## 4. 收尾

- [x] 4.1 跑相關 backend 測試與前端 build，確認既有 MR agent-turn 行為未回歸。
