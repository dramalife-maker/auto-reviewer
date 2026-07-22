# 知識缺口：本機 reviewer-server 在跑時，`cargo test` 會因 exe 被鎖而 build 失敗；測試須用獨立 target dir

<tl;dr>
- **何時要想起這則：** 在本機開發時要跑 `cargo test -p reviewer-server`（尤其同時有 `cargo run -p reviewer-server` 或已部署的 server 正在跑）。
- **不要做：** 直接 `cargo test`（預設寫 `target/`）。若 `target\debug\reviewer-server.exe` 正被執行中的 server 佔用，link 階段要覆寫 exe 會失敗。
- **要做：** 測試分流到獨立 target dir：PowerShell `\$env:CARGO_TARGET_DIR="target-test"; cargo test -p reviewer-server`。`target-test/` 已在 `.gitignore`，是專案既有慣例。
- **症狀：** `error: failed to remove file target\debug\reviewer-server.exe` / `Caused by: 存取被拒。 (os error 5)`，build failed，但程式碼其實沒錯。
- **自問（可選）：** 這個 exe 現在是不是正在跑？我的測試和正式 build 有沒有共用同一個 `target/`？
</tl;dr>

## 問題描述

實作 `manual-person-rerun` 後跑 `cargo test -p reviewer-server` 驗證，link 階段直接失敗：

```
error: failed to remove file `E:\...\target\debug\reviewer-server.exe`
Caused by:
  存取被拒。 (os error 5)
```

一開始看起來像編譯錯誤，實際是 Windows 檔案鎖：正式 build 的 `reviewer-server.exe` 正被一個執行中的 server process 佔用，cargo 在 link 階段要覆寫同名 exe 就被 OS 拒絕。與程式碼、依賴、測試內容都無關。

## 錯誤原因／學到的知識

| 層 | 行為 |
|----|------|
| Windows | 執行中的 `.exe` 被鎖，任何進程（含 cargo linker）不能刪除／覆寫該檔 |
| cargo | 預設所有 profile（`run` 與 `test`）共用同一個 `target/`；test 的 integration binary link 前會嘗試更新 `target/debug/reviewer-server.exe` |
| 結果 | 只要 server 在跑，測試就無法在同一 `target/` 完成 link |

這是專案既有慣例（archived change `2026-07-09-mr-inbox-dedup/tasks.md` 有寫「必要時 `CARGO_TARGET_DIR=target-test`」、`.gitignore` 也 ignore 了 `target-test/`），但沒寫進 README 的「## 測試」段，第一次進場的 agent／新人容易撞到。

## 解決方法

測試用獨立 target dir，與正式 build（`target/`）分流：

```powershell
$env:CARGO_TARGET_DIR="target-test"; cargo test -p reviewer-server
```

如此測試 link 到 `target-test/debug/...`，不去碰被鎖的 `target/debug/reviewer-server.exe`。

## 避免方法

- 本機一律用 `CARGO_TARGET_DIR=target-test` 跑測試，別跟正式 build 搶 `target/`。
- 若堅持用同一 `target/`，先停掉正在跑的 server 再測。
- 看到 `os error 5 / 存取被拒 / failed to remove file *.exe`，先想「這 exe 是不是正在跑」，別誤判成程式碼問題。

## 相關檔案

- `.gitignore`（已 ignore `target-test/`）
- `README.md`（「## 測試」段——本次補一行說明）
- `backend/Cargo.toml`（package `reviewer-server`，同時產出 bin 與 integration tests）
