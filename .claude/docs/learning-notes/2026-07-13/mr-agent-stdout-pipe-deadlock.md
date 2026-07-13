# 知識缺口：MR agent piped stdout 未邊等邊讀 → pipe 死鎖假 timeout

<tl;dr>
- **何時要想起這則：** 子行程 `stdout(Stdio::piped())` + `child.wait()`／timeout，且子行程會大量寫 stdout（例如 `stream-json`）；手動 CLI 幾分鐘結束、server 卻卡滿 `per_project_timeout`；log 裡 `stdout_bytes` 卡在約 40–64KiB。
- **不要做：** 等 `wait()`／timeout 之後才 `read_to_end` stdout／stderr。
- **要做：** spawn 後立刻把 pipes 丟給並行 drain task，再 race wait／timeout／cancel；timeout 時仍先 `kill` 再 `join` drain。
- **意圖：** 避免 agent 寫滿 OS pipe buffer（Windows 約 64KiB）後阻塞寫入，父行程又在等退出 → 死鎖，被誤判成「審查太慢」。
</tl;dr>

## 證據

- 手動重跑同一 `cursor-agent` 指令：約 3 分鐘結束。
- server 同一 MR：`SkippedTimeout` 600s；`stdout_bytes≈49–54KiB`（貼近 pipe buffer）。
- 根因：`execute_mr_review` 先 `wait_with_cancel`，結束後才讀 pipe。

## 修復

`backend/src/executor.rs`：`spawn_stdout_drain`／`spawn_stderr_drain` 與 wait 並行；測試 `execute_mr_review_drains_stdout_while_waiting`（fixture 寫 200KiB 再 exit 0）。
