## 1. Schema

- [x] 1.1 依「獨立訊息表而非 JSON 欄位」新增 migration `013_mr_review_chat_messages.sql`（含 FK CASCADE 與 `mr_review_id` index），使 Persist successful Agent Chat turns in SQLite 的儲存契約可落地；驗證：測試 DB 套用 migration 成功且 `schema_version` 含 13

## 2. Backend persistence and API

- [x] 2.1 實作「僅成功回合落庫」：`agent_turn` 成功後插入 user 再 assistant；失敗不寫入，寫入失敗則整體失敗。覆蓋 Persist successful Agent Chat turns in SQLite。驗證：`backend/tests/mr_reviews.rs` 成功與失敗案例
- [x] 2.2 實作「列表嵌入 `chat_messages`」與 `draft_hash`：`list_mr_reviews` 回傳依 id ASC 的 `chat_messages` 與 body 的 SHA-256 `draft_hash`。覆蓋 List API returns chat transcript per review。驗證：對 `GET /api/mr-reviews` JSON 斷言
- [x] 2.3 確認「生命週期與唯讀規則」後端面：publish／ignore 不刪訊息，非 draft 的 agent-turn 仍 409。覆蓋 Publish and ignore retain chat history。驗證：publish 後 list published 仍含同一 transcript
- [x] 2.4 實作「Agent 改草稿為預期行為」：成功 turn 後重讀檔案，回應含 `draft_body` 與 `draft_hash`。覆蓋 Agent turn returns current draft body。驗證：測試在 turn 前後改檔／不改檔兩種回應
- [x] 2.5 實作「PATCH 樂觀鎖定與 `draft_hash`」：可選 `base_hash`；不符則 409 且不寫檔並回傳目前 draft。覆蓋 Draft PATCH supports optimistic base hash。驗證：錯 hash → 409；對 hash → 寫入成功

## 3. Frontend hydrate, read-only, and draft conflict UI

- [x] 3.1 型別與列表載入帶上 `chat_messages`／`draft_hash`；選取時 hydrate Agent Chat。覆蓋 MR inbox hydrates Agent Chat from the API。驗證：`MrInboxPage.test.tsx` 載入含歷史的 draft 會渲染訊息
- [x] 3.2 [P] published／ignored 有歷史時唯讀、無送出；無歷史則隱藏。覆蓋 Published and ignored Agent Chat is read-only。驗證：同測試檔 published fixture
- [x] 3.3 實作「基準草稿與衝突 UX」：baseline、新版本標記、dirty 衝突三動作（預覽新版本／載入／保留）、預覽唯讀不改 editor、dirty 時不因 state 更新重置編輯器。覆蓋 Draft editor tracks server baseline and new versions。驗證：`MrInboxPage.test.tsx` 未 dirty 套用、dirty 衝突、預覽唯讀三案例
- [x] 3.4 儲存帶 `base_hash`；409 走同一衝突 UI（含預覽）。覆蓋 MR inbox save sends base hash。驗證：mock PATCH 409 時出現 Preview／Load／Keep

## 4. Regression

- [x] 4.1 跑後端 `mr_reviews` 測試與前端 `MrInboxPage` 測試，確認 publish／ignore／無 session draft 未破。驗證：專案慣用 `cargo test`／`npm test` 指令通過
