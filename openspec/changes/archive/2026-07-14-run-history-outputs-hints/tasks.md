## 1. Backend outputs API

- [x] 1.1 實作 Decision: MR 草稿計數來源為 drafts 目錄 — 提供從 `mr_poll_draft_dir` 計算 `*.md` 數量的 helpers；目錄缺失／不可讀回傳 0。驗證：單元測試斷言 0／N 個檔案的計數。
- [x] 1.2 實作 Decision: 週報來源為 reports.run_id — 依 `(run_id, project_id)` 查 `reports` JOIN `people`，回傳 `person_id` + `display_name`。驗證：單元或整合查詢在有／無列時回傳正確清單。
- [x] 1.3 實作 Decision: API 形狀與省略規則 與 Decision: outputs 以讀取時衍生、不落庫 — `get_run` 在 run 非 `running` 時組裝每專案 `outputs`（滿足 **Run detail includes project outputs summary**）；`running` 省略。驗證：擴充 `backend/tests/runs_execution.rs` — finished MR 有 drafts → count；finished weekly 有 reports → people；running → outputs null／省略；缺 drafts 目錄 → 無 `mr_drafts` 且 HTTP 200。

## 2. Frontend 導向提示

- [x] 2.1 [P] 擴充 `RunProjectStatus` 型別對應 `outputs` JSON 形狀。驗證：TypeScript 編譯通過（`frontend` typecheck／既有 test 指令）。
- [x] 2.2 實作 Decision: UI 文案與連結，滿足 **Execution history detail shows outputs navigation hints**：專案結果卡顯示 MR／週報提示與 `/mr-inbox`、`/reports/{personId}` 連結；超過 8 人名截斷；無 outputs 不渲染區塊。驗證：前端測試覆蓋 MR hint、週報人名連結、無 outputs 不顯示。

## 3. 測試與收斂

- [x] 3.1 實作 Decision: 測試落點 — 補齊 1.3／2.2 所列 backend／frontend 測試並全部通過。驗證：執行對應 cargo／vitest（或專案慣用）指令綠燈。
