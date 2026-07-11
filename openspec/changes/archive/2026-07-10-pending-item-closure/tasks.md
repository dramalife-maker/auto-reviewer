## 1. Backend module and list/resolve APIs

- [x] 1.1 新增 pending-items 模組，實作 Closure API 的列表端點 `GET /api/people/{id}/pending-items`（預設 `status=open`，支援 `resolved`/`all`），回傳含 `id`/`project_name`/`question`/`status` 等欄位的陣列；未知 person 回 404。對齊 design「Closure API」與 spec「Pending items can be listed for a person」。驗證：整合測試覆蓋預設只回 open、`?status=resolved`、404。
- [x] 1.2 實作 Closure API 的 `PATCH /api/pending-items/{id}`：僅允許 `open → resolved`；`resolved_date` 依 `schedule_config.tz_offset_min` 取當月 `YYYY-MM`；可選 `resolution_note`；已 resolved 回 409、非法 status 回 400、未知 id 回 404。對齊 design「DB-first 閉環，檔案為衍生視圖」「resolved_date 使用排程時區當月」「Closure API」，以及 spec「Open pending items can be resolved via API」。驗證：整合測試覆蓋成功閉環、帶 note、409、400。
- [x] 1.3 在 router 註冊 Closure API 兩個端點並匯出模組。驗證：`cargo test` 相關整合測試可打到真實 HTTP handler 路徑。

## 2. Notes file sync and trends parsing

- [x] 2.1 實作 `_notes.md` B1 同步：閉環 DB 成功後改寫／append／建檔；匹配第一筆 question 全等的 open 行；檔案失敗回 502 且 DB 保持 resolved。對齊 design「DB-first 閉環，檔案為衍生視圖」與「`_notes.md` B1 行格式與匹配規則」，以及 spec「Resolving a pending item syncs person-level notes file」。驗證：單元或整合測試覆蓋改寫、append、建檔、502。
- [x] 2.2 更新 trends reader：`historical_pending` 改為結構化物件（`question`/`status`/`raised_month`/`resolved_month`/`resolution_note`/`raw_line`），正確解析 open 與 resolved B1 行。對齊 design「趨勢 historical_pending 結構化」與 spec「Person trends read API」「Person-level report directory layout」。驗證：更新 `person_trends` 測試斷言結構化欄位與兩種行格式。

## 3. Latest reports contract and ingestion dedupe

- [x] 3.1 實作 Latest reports pending shape：修改 `GET /api/people/:id/reports/latest`，各專案卡以 DB open `pending_items` 取代 summary 字串 `pending: string[]`（本週 API 以 `pending_items` 取代 summary 字串）；元素含 `id` 與 `question` 等。對齊 design「本週 API 以 `pending_items` 取代 summary 字串」「Latest reports pending shape」與 spec「Latest weekly report content is served per person」。驗證：更新 `report_reader` 整合測試斷言新欄位且 resolved 不出現。
- [x] 3.2 週報 summary ingestion 對同 `person_id`+`project_id`+`question` 且仍 open 的列去重；已 resolved 同文可再插入新 open。對齊 design「Ingestion 去重僅針對仍 open 的同文問題」與 spec「Weekly summary ingestion deduplicates open pending questions」。驗證：整合或單元測試覆蓋 skip 與再提出兩條路徑。

## 4. Frontend checkbox closure

- [x] 4.1 更新前端型別與 API client：latest reports 使用 `pending_items`；新增 resolve pending-item 呼叫；trends `historical_pending` 改為物件陣列。驗證：`npm run build` 通過型別檢查。
- [x] 4.2 本週專案卡待確認改為 checkbox；勾選呼叫 PATCH；成功後移除該項並刷新人員 open-pending badge（及已載入的 dashboard pending 計數）。趨勢 Tab 歷史待確認僅唯讀區分 open／resolved 樣式，不提供閉環。對齊 spec「Weekly report UI resolves pending items via checkbox」。驗證：手動或前端邏輯斷言勾選觸發 PATCH；`npm run build` 成功。

## 5. Docs sync

- [x] 5.1 更新 `docs/idea/schema.md` 與 `README.md`：記載閉環 API、B1 `_notes.md` 格式、latest reports `pending_items` breaking 變更。驗證：文件內容審查含端點與行格式範例。


