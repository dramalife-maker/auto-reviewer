## 1. Manifest 注入 open pending

- [x] 1.1 實作 Manifest 注入 open pending：`write_weekly_manifest` 產出的週報 `manifest.json` 必須含 `open_pending` 陣列（元素 `id`/`person_id`/`display_name`/`question`），內容為該專案所有 `status='open'` 的 pending，排序 `person_id` 再 `id`；無資料時為空陣列。驗證：新增或擴充整合測試斷言有／無 open 列時的 JSON 形狀，並排除 resolved。涵蓋需求 Weekly batch manifest includes open pending items。

## 2. Workflow 契約

- [x] [P] 2.1 實作 Workflow 沿用或省略：更新 `skills/reviewer-batch/WORKFLOW.md`（及必要時 `output-contract.md`），規定讀取 `manifest.open_pending`；延續議題必須原句寫入 `## 待確認`，不再相關可省略且不 resolve。驗證：文件內容審查含上述硬性規則與 manifest 欄位說明。涵蓋需求 Reviewer-batch reuses open pending question text verbatim 與設計決策 Workflow 沿用或省略。

- [x] [P] 2.2 更新 `docs/idea/schema.md` §4.3 週報 manifest 範例／欄位表，記載 `open_pending` 形狀與語意。驗證：文件審查可見該欄位與元素鍵名。

## 3. 測試策略收斂

- [x] 3.1 完成測試策略：確認週報 manifest 整合測試涵蓋 open pending 注入（對齊設計「測試策略」）；執行相關 `cargo test`（至少 identity／manifest 相關測項）通過。驗證：測試綠燈且斷言覆蓋空陣列與含一筆 open 的案例。
