<!--
Each task description MUST state behavior + verification.
File paths are locator context only.
-->

## 1. Skill 與檔案契約

- [x] 1.1 建立 `skills/project-adr-notes/` 契約：定義 **Project ADR directory layout** 與 **ADR index and body contract**；落實 design decision「ADR 目錄採 index + 單則檔（非單檔全文）」、「ADR 正文用 `<tl;dr>`，不用 YAML／`<meta>`」、以及「index 更新規則對齊 post-bug（禁止整檔覆寫）」（H1 + `<tl;dr>` 僅掃描鍵：何時想起／決策／不要做／要做／意圖；**不**寫 date·status·source·mr_iid；index 僅追加列）。驗證：契約範例 ADR 的 `<tl;dr>` 不含那四個鍵；`rg "<tl;dr>|禁止整" skills/project-adr-notes`；正式格式無 YAML。
- [x] 1.2 落地 **Explicit write trigger only** 與 design decision「寫入觸發僅顯式指令（v1）」（白名單含「記成 ADR」「寫入決策」「record as ADR」；無指令不寫）。驗證：契約含正／反例各至少一則；`rg "記成 ADR|寫入決策|record as ADR" skills/project-adr-notes`。

## 2. Manifest 與 executor

- [x] 2.1 實作 design decision「`notes_dir` 由後端寫入 manifest 與 agent-turn 上下文」之 manifest 部分：滿足 **Weekly and MR manifests include notes_dir**（路徑 `{DATA_ROOT}/reports/{project}/.notes`）。驗證：backend 測試 assert weekly／mr_poll `manifest.json` 皆含正確 `notes_dir`。
- [x] [P] 2.2 實作 design decision「Agent-turn 附加 ADR skill，不改 draft／發佈語意」：滿足 **Agent-turn receives ADR skill and notes_dir**（附加 skill、turn 上下文含 `notes_dir`；不強制改 draft、不發佈）。驗證：command builder 單元測試 assert Claude argv 含 skill 路徑；turn 上下文含 `.notes`。

## 3. 兩軌 workflow 必讀

- [x] [P] 3.1 落實 design decision「兩軌必讀與禁止重問寫在 workflow，不重建 pending 去重」於週報：滿足 **Reviewer workflows consume notes_dir** 與 **Headless tracks must read ADRs before asking**（讀 `manifest.notes_dir`／index；已知技術選擇不得進新 `## 待確認`；headless 不寫 ADR）。驗證：`rg "notes_dir|待確認" skills/reviewer-batch/WORKFLOW.md skills/reviewer-batch/output-contract.md`；WORKFLOW checklist 含該項。
- [x] [P] 3.2 同上於 MR scan：消費 `notes_dir`，建議追問不得重問已知 ADR。驗證：`rg "notes_dir|建議追問" skills/scan-mrs-headless/WORKFLOW.md skills/scan-mrs-headless/output-contract.md`。

## 4. 保留目錄與邊界

- [x] 4.1 落實 design decision「`.notes` 為專案報告根下保留目錄名」與 **Project .notes directory is reserved metadata**（不當人物資料夾；pending 仍寫 `_people/.../_notes.md`）。驗證：列舉邏輯 skip `.notes` 或 WORKFLOW／註解明示；`rg "\\.notes" skills/` 可追溯。

## 5. 驗證收斂

- [x] 5.1 跑相關 `cargo test`（manifest／executor／既有 MR agent-turn 不回歸）。驗證：測試通過。
