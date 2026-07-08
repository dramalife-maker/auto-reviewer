# 需求誤解：Reviewer model 環境變數不應綁定單一 agent

<tl;dr>
- **何時要想起這則：** 新增 reviewer / agent CLI 相關 env，或要在 headless 子進程傳 `--model` 時。
- **不要做：** 用 `REVIEWER_CURSOR_MODEL` 等 agent 專用名稱；未設定 model 仍硬傳 `--model`；在文件暗示只有某一 agent 才支援 model。
- **要做：** 用通用 `REVIEWER_MODEL`，經 `AppConfig::reviewer_model()` 讀取；僅在 env 有非空值時才 `append_model_arg`；claude / cursor 共用同一 helper。
- **意圖：** 切換 `REVIEWER_AGENT` 時設定介面不變；model 語意由使用者依各 CLI 文件自行選擇，框架不替 agent 背書。
- **自問（可選）：** 這個 env 名稱在換成另一個 agent 時還合理嗎？沒設值時 CLI 會不會被多塞一個無意義參數？
</tl;dr>

## 使用者為何希望這樣改（意圖）

使用者會在 `REVIEWER_AGENT` 之間切換 claude / cursor，並**自行知道**各 agent 支援哪些 model。環境變數不必（也不應）在名稱上綁死 Cursor；未指定 model 時應讓 CLI 用預設，不要多傳 `--model`。

## 問題描述

初版使用 `REVIEWER_CURSOR_MODEL`，僅在 cursor 路徑讀取並傳給 `cursor-agent --model`。切換到 claude 時沒有對應欄位，且命名暗示 model 是 cursor 專屬能力。

## 錯誤原因

把「目前預設 agent 是 cursor」誤寫成「model 設定是 cursor 專屬設定」；未區分「可選覆寫」與「必須傳參」。

## 解決方法

1. `config.rs`：`REVIEWER_MODEL` → `reviewer_model: Option<String>`，空字串視為未設定。
2. `executor.rs`：`append_model_arg()` 在 claude / cursor 建 command 時共用；`Some(model)` 才加 `--model`。
3. `.env.example` / `README.md`：改為 agent 中立的 `REVIEWER_MODEL` 說明。

## 避免方法

- 多 agent 架構下，env 優先命名 **能力**（model、executor），不要命名 **供應商**（cursor_model），除非該變數真的只對單一 provider 有意義。
- CLI 可選參數：**未設定就不傳**，避免空 flag 或錯誤預設。
- 文件只說「對應 agent CLI 的 `--model`」，不列舉各 agent 型號清單。

## 相關檔案

- `backend/src/config.rs`
- `backend/src/executor.rs`
- `.env.example`
- `README.md`
