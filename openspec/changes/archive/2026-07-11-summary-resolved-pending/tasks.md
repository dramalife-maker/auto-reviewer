## 1. Summary 契約與解析

- [x] 1.1 更新 output-contract／WORKFLOW（含設計「Workflow 規則更新」）：`summary.md` 必須含 `## 已釐清`；已釐清寫原文、不得同時出現在待確認；僅省略兩區則不請求閉環。驗證：文件審查含四 heading 與延續／已釐清規則。涵蓋需求 Weekly summary includes resolved section for closed pending。

- [x] 1.2 實作 summary 解析 `## 已釐清`：`parse_summary_file` 產出 `resolved_questions`（bullet 文字列表）；空區為空 vec。驗證：單元或整合測試用含／不含該區的 fixture。對齊設計「已釐清訊號用固定 heading」。

## 2. Ingest 自動閉環

- [x] 2.1 實作 Ingest 呼叫共用 resolve 路徑：ingest upsert 後對 `resolved_questions` 精確匹配 open `(person_id, project_id, question)` 並 resolve（`resolved_date` schedule 月、`resolution_note` null、嘗試 `_notes.md`）；無匹配 warn 忽略；notes 失敗 warn 且繼續。驗證：整合測試「Exact open question in 已釐清 becomes resolved」與「待確認 omission without 已釐清 does not resolve」。涵蓋需求 Summary ingestion auto-resolves matching open pending from 已釐清 與 Weekly ingest resolves open pending via shared closure semantics。

- [x] [P] 2.2 更新 `docs/idea/schema.md`：記載 `## 已釐清` 與 ingest 自動 resolve 語意。驗證：文件審查可見該 heading 與閉環說明。

## 3. 回歸

- [x] 3.1 跑相關 `cargo test`（summary／pending／runs_execution／identity 等受影響測項）全部通過。驗證：指令綠燈。
