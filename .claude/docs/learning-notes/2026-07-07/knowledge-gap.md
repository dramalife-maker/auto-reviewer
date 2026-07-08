# 知識缺口：整合 Cursor Agent CLI 作為 headless reviewer

<tl;dr>
- **何時要想起這則：** 整合或維護 Cursor CLI reviewer、撰寫多行 prompt 給 subprocess、部署 headless 伺服器需認證時。
- **不要做：** 把 Cursor 當純 HTTP completion API；假設 `--append-system-prompt-file` 存在；在 Windows 直接傳含 LF 的多行 prompt 給 `cmd.exe`；依賴與 reviewer-server 不同使用者的 login token。
- **要做：** 用 `cursor-agent --print --output-format stream-json --trust --force [--model]` + positional prompt；workflow/contract 內嵌至 prompt；Windows 多行走 `prepare_prompt_for_cli`（字面 `\n`）；以已 `cursor-agent login` 的同一使用者啟動 reviewer-server（子行程繼承 token）；`REVIEWER_AGENT=cursor|claude` 切換 executor。
- **意圖：** 從 Claude 遷移／並行支援 Cursor CLI 作為預設 agent，沿用既有 reviewer-batch workflow。
- **自問（可選）：** 本專案只需 wait exit code 還是要 parse NDJSON stream？Windows 上 prompt 是否已轉義？
</tl;dr>

## 使用者為何希望這樣改（意圖）

使用者希望從 Claude Code CLI **遷移或並行支援** Cursor Agent CLI 作為預設 reviewer executor，利用既有 reviewer-batch workflow，而不必重寫整條審查流程。

## 問題描述

需整合 Cursor Agent CLI（`cursor-agent`）作為 headless reviewer executor，與既有 Claude Code CLI 並存。

## 錯誤原因／學到的知識

- Cursor 是 **local subprocess provider**，非 HTTP API；headless 需 `--print --trust --force`。
- **prompt 為 positional argument**，非 flag。
- `stream-json` 為 **NDJSON stdout**；本專案僅 wait exit code，不需 parse stream。
- Cursor **無** `--append-system-prompt-file`，workflow 需**內嵌至 prompt**。
- **Windows：** `cmd.exe` 會在 LF 截斷參數，多行 prompt 需轉成字面 `\n`。
- **Headless 認證：** 子行程繼承啟動 reviewer-server 的使用者環境；本機預先 `cursor-agent login` 即可（實測無 `CURSOR_API_KEY` 亦可）。token 過期時 stderr 含 `Authentication required`，後端標記 failed、前端紅色 banner 提示重新 login。
- 參考文件：`E:\workspace\github\Claude-Code-Mini-App\docs\spec\cursor-agent-cli.md`

## 解決方法

- 環境變數 `REVIEWER_AGENT=cursor|claude`（預設 cursor）。
- Cursor 路徑：`cursor-agent --print --output-format stream-json --trust --force [--model]` + 內嵌 WORKFLOW/contract。
- Windows 多行 prompt 使用 `prepare_prompt_for_cli` 處理。

## 避免方法

- 不要把 Cursor 當純 completion API。
- 本機部署以已 `cursor-agent login` 的使用者啟動 reviewer-server；認證失敗會寫入 `run_projects.error` 並由前端 banner 提示。
- Windows 多行 prompt 走 `prepare_prompt_for_cli`，勿直接傳含換行的字串給 CLI。

## 相關檔案

- `backend/src/executor.rs`
- `.env.example`
- `README.md`
