# 架構缺陷／知識缺口：Windows 上 cursor-agent spawn 失敗導致 run 永久卡在 running

<tl;dr>
- **何時要想起這則：** 在 Windows 用 Rust `std::process::Command` 啟動 CLI（`cursor-agent`、`npm`、任何 `.cmd`/`.bat` shim）時；設計「排入佇列 → 背景 worker 執行 → 更新狀態」這類 job pipeline 時；看到 job handler 用 `?` 直接往外拋錯時。
- **不要做：** 用 `Command::new("cursor-agent")` 直接 spawn（Windows 只補 `.exe`，找不到 `.cmd`）；也**不要**只解析到 `.cmd` 就交給 `Command`——該 shim 內部 `powershell -File *.ps1 %*` 會對含 `"`／metachar 的大 prompt **二次解析**而打散（PowerShell 報 `argument name is not valid`）；job handler 用 `?` 讓執行錯誤跳出而不落地終態；`JoinHandle` 迴圈只處理 `JoinError` 忽略內層 `Err`；timeout 只 `child.kill()`（留 node 孤兒）。
- **要做：** 用 `which` 解析 shim，若是 `.cmd`/`.bat` 則找同名 `.ps1` 改用 `powershell.exe -NoProfile -ExecutionPolicy Bypass -File <ps1> <args>` 直接呼叫（**只剩單層解析**，任意 prompt 內容都安全）；真正 `.exe`（claude）直接 spawn；job handler 任何失敗路徑都要 `finish_run_project(failed)` + `finalize_run_if_complete`；join 迴圈要同時 log 內層 `Err`；Windows kill 用 `taskkill /F /T /PID`；認證失敗從 stderr 辨識並回傳給前端 banner。
- **意圖：** 讓「全部執行」在 Windows 開發環境能真正跑起來，且任何失敗都能明確反映到 UI（不再假死顯示「執行中」）。
- **自問（可選）：** 這個外部程式在 Windows 是 `.exe` 還是 `.cmd`？這條錯誤路徑會不會讓 DB 狀態卡住？子行程被 kill 後 node 會不會變孤兒？
</tl;dr>

## 使用者為何希望這樣改（意圖）

使用者按下「全部執行」後，UI 永遠顯示「執行中」，無法分辨是仍在跑還是壞掉。希望找出 cursor-agent spawn 的問題並修好，讓執行要嘛成功、要嘛明確失敗，不要假死。

## 問題描述

`POST /api/runs` 後，`runs.status` 與 `run_projects.state` 永久停在 `running`：無子行程、無 timeout、log 也無任何錯誤，UI 一直卡在「執行中」。

## 錯誤原因（分兩批修，共四層）

第一批（假死）：

1. **Windows `.cmd` 找不到**：`cursor-agent` 在 Windows 只有 `.cmd`（無 `.exe`）。`Command::new("cursor-agent")` 走 `CreateProcess`、只補 `.exe` → spawn 失敗。（`claude` 是 `.exe` 故無事。）
2. **失敗未落地終態**：`process_run_project` 用 `?` 直接拋出，跳過 `finish_run_project`/`finalize_run_if_complete`，`run_projects.state` 停在 `running`。
3. **內層錯誤被吞**：`drain_queue` 只捕捉 `JoinError`，`Ok(Err(app_error))` 的內層 `Err` 被丟棄，無 log。

第二批（真正的 spawn 根因，補完第 1 點後才浮現）：

4. **`.cmd` shim 二次解析打散大 prompt**：`which` 解析到 `cursor-agent.cmd`，其內容為 `powershell.exe -File cursor-agent.ps1 %*`。Rust 對 `.cmd` 依批次規則轉義（第一層 cmd.exe），但 shim 的 `%*` 把參數**再交給 PowerShell `-File` 解析第二層**。我們的 reviewer prompt 內嵌整份 WORKFLOW+contract（大量 `"`、backtick、`{}`），兩層解析對不上被打散，PowerShell 最終把碎片當成參數名 → `Cannot process argument because the value of argument name is not valid`。實測：`.NET ArgumentList` 直呼 `.cmd`（單層正確轉義）可跑；`powershell -File .ps1` 直呼（單層）亦可跑；唯獨經 Rust→cmd→`%*`→powershell 這條雙層鏈會爆。

## 解決方法

- **A（改良）**：`reviewer_command()` 用 `which` 解析程式；Windows 上若解析到 `.cmd`/`.bat`，改找同名 `.ps1` 並用 `powershell.exe -NoProfile -ExecutionPolicy Bypass -File <ps1>` 直接呼叫，消除 `.cmd` 內部 `%*` 的第二層解析（單層 → 任意 prompt 安全）。`.exe`（claude）維持直接 spawn。LF 仍由 `prepare_prompt_for_cli` 轉字面 `\n`。
- **B**：`process_run_project` 把 `execute_weekly_batch` 的 `Err` 轉為 `finish_run_project(failed)` + `finalize_run_if_complete` 再 `return Ok(())`；`ingest_project_summaries` 失敗亦標記 failed。`drain_queue` 改 `match handle.await` 同時 log 內層 `Err`。
- **C**：timeout 時 `taskkill /F /T` 殺整棵樹。
- **認證失敗**：executor 讀 stderr 辨識 `Authentication required`；`GET /api/runs/:id` 回傳 `projects[].error`；前端紅色 banner 提示重新 login。

## 診斷方法（值得複用）

用 `.NET ProcessStartInfo` + `ArgumentList`（正確 Windows 轉義）逐步縮小：先測旗標、再測含 metachar 的 prompt、最後比對 `.cmd` 直呼 vs `powershell -File .ps1` 直呼。哪條鏈爆、哪條過，根因立現——不必臆測。

## 避免方法

- Windows 上啟動外部 CLI 前，先確認是 `.exe` 還是 `.cmd`。`.cmd` shim 常內含 `powershell -File *.ps1 %*` 的二次解析，若要傳含引號/metachar 的大字串，別讓 Rust 經 `.cmd`；改直呼底層 `.ps1`（`powershell -File`）或 `.exe`，維持單層解析。
- 背景 job 的每一條失敗路徑都必須把狀態收斂到終態；**禁止**用 `?` 讓 job handler 直接拋錯而不更新 DB。
- `JoinHandle` 迴圈要同時處理 `JoinError` 與內層 `Result::Err`，不可只看外層。
- Windows kill 子行程請殺整棵樹（`taskkill /F /T`），避免 node 孤兒。

## 相關檔案

- `backend/src/executor.rs`（`reviewer_command`、`kill_process_tree`、`format_executor_failure`、`build_cursor_command`/`build_claude_command`）
- `backend/src/worker.rs`（`process_run_project`、`drain_queue`）
- `backend/Cargo.toml`（新增 `which`）
- `frontend/src/app.ts`（run 完成 banner、認證失敗提示）
