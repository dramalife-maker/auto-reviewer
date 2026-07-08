# 架構缺陷：Reviewer 環境變數散落在 executor 模組

<tl;dr>
- **何時要想起這則：** 新增或修改任何環境變數（尤其 reviewer、executor、worker 相關）時；在 `executor.rs` / `worker.rs` 看到 `std::env::var` 時。
- **不要做：** 在業務模組內定義 env 常數並直接 `std::env::var` 讀取；讓 `REVIEWER_AGENT`、`REVIEWER_CURSOR_MODEL` 等與 `PORT`、`DATA_ROOT_DIR` 管理方式不一致。
- **要做：** 先查 `config.rs` 是否已有同類模式；常數、enum、預設值、`from_env()` 載入與 getter 一律集中在 `AppConfig`；executor/worker 只透過 `config.reviewer_agent()` 等介面使用。
- **意圖：** 環境設定統一管理，降低維護與測試成本，與專案既有 config 慣例一致。
- **自問（可選）：** 這個 env 是否已在 `config.rs`？測試用的 `REVIEWER_EXECUTOR` 是否也該經 config 而非散落讀取？
</tl;dr>

## 使用者為何希望這樣改（意圖）

使用者希望所有環境變數設定**統一管理**，避免散落在各業務模組難以維護與測試。專案既有慣例是 `PORT`、`DATA_ROOT_DIR` 等集中在 `config.rs` 的 `AppConfig::from_env()`；reviewer 相關設定也應遵循同一模式，而非在 executor 內各自為政。

## 問題描述

`REVIEWER_AGENT`、`REVIEWER_CURSOR_MODEL` 環境變數常數與讀取邏輯散落在 `backend/src/executor.rs`，與專案其他 env（`PORT`、`DATA_ROOT_DIR` 等集中在 `config.rs`）不一致。

## 錯誤原因

新增 Cursor CLI 支援時，直接在 executor 模組內定義 `const REVIEWER_AGENT_ENV` 並用 `std::env::var` 讀取，未遵循既有 `AppConfig::from_env()` 模式。

## 解決方法

1. 將常數、`ReviewerAgent` enum、`reviewer_cursor_model`、`reviewer_executor` 移入 `config.rs`。
2. 在 `AppConfig::from_env()` 啟動時載入上述設定。
3. `executor.rs`、`worker.rs` 改透過 `config.reviewer_agent()` 等 getter 使用，不再直接讀 env。

## 避免方法

- 新增任何環境變數時，**先查 `config.rs` 是否已有同類模式**。
- **禁止**在業務模組（executor/worker）直接 `std::env::var`。
- 測試用 `REVIEWER_EXECUTOR` 亦應經 config 注入，而非在模組內硬讀。

## 相關檔案

- `backend/src/config.rs`
- `backend/src/executor.rs`
- `backend/src/worker.rs`
