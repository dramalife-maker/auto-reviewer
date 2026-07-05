## Context

專案已有產品設計文件 `docs/idea/spec.md` 與資料 schema `docs/idea/schema.md`，定義雲端雙軌 1on1 reviewer（本 MVP 僅實作**軌道 1 週報**核心流程）。後端 Rust 常駐、SQLite + 檔案、`DATA_ROOT_DIR` 布局、skill 子行程邊界均已拍板。目前 repo 無應用程式碼，僅 Spectra / OpenSpec 脚手架。

## Goals / Non-Goals

**Goals:**

- 交付可部署的 Rust HTTP API + 瀏覽器 SPA。
- 啟動時讀取 `projects.yaml` 與環境變數，初始化 DB 與資料目錄。
- 支援「全部執行」與排程觸發的 reviewer-batch 子行程，解析 `summary.md` 入庫。
- 提供本週報告閱讀 API 與最小可用 UI（左欄人員、本週內容、已讀）。
- 執行完成後前端可見通知（polling `/api/runs/latest` 狀態變更即可）。

**Non-Goals:**

- 趨勢 Tab、MR review 收件匣（軌道 2）、人員/專案設定 UI。
- 多使用者認證（MVP 無 auth 或僅 reverse proxy 層）。
- `git_remote_url` 自動 clone（僅存欄位，手動準備 `repo_path`）。
- Telegram、數量型圖表、漏跑補償 UI。

## Decisions

### Rust 後端框架選用 axum + sqlx + tokio

- **選擇**：axum 0.7、sqlx（SQLite）、tokio 全異步。
- **理由**：與 spec 中 worker pool / cron 一致；sqlx 可版本化 migration。
- **替代**：actix-web（生態相當，本專案無偏好）。

### 前後端目錄結構

- **選擇**：
  - `backend/` — Cargo workspace 單 crate `reviewer-server`
  - `frontend/` — Vite + TypeScript + React（或 Svelte，實作時擇一並在 tasks 鎖定）
  - `projects.yaml` — repo 根目錄範例
- **理由**：與 proposal Impact 一致，利於獨立 build / deploy。

### summary.md 格式

- **選擇**：獨立 `summary.md` + YAML frontmatter（schema §7）。
- **理由**：與 schema 已定格式一致；後端解析 frontmatter 寫 `reports` / `pending_items`。

### MVP 人員來源

- **選擇**：首次執行時由 summary frontmatter `person` 自動 upsert `people`；無 `person_identities` UI。
- **理由**：MVP 無人員設定頁；identity 歸戶留 v2。

### 排程實作

- **選擇**：`tokio-cron-scheduler` 讀 `schedule_config` 表；服務啟動時 seed 預設列。
- **理由**：spec §7.3 已決。

## Implementation Contract

### 後端啟動

- **行為**：設定 `DATA_ROOT_DIR` 後，程序啟動 MUST 建立 `reviewer.db`、執行 migration、建立 `repos/` 與 `reports/` 根目錄（若不存在）。
- **API**：`GET /health` 回 `{ "status": "ok", "data_dir": "<path>" }`。
- **失敗**：`DATA_ROOT_DIR` 未設定 → 程序 exit code 非 0，stderr 訊息含變數名。
- **驗收**：`curl /health` 回 200；目錄存在。

### projects.yaml 載入

- **行為**：啟動時讀 repo 根 `projects.yaml`（路徑可經 env `PROJECTS_CONFIG` 覆寫），upsert `projects`（`name`, `repo_path`, `git_remote_url`）；對每個 `repo_path` 執行 git 偵測更新 `is_git_repo`, `default_branch`。
- **失敗**：yaml 語法錯誤 → 啟動失敗；`repo_path` 非目錄 → `is_git_repo=0` 仍寫入，執行時 skip 並記 `run_projects.error`。
- **驗收**：integration test 載入範例 yaml 後 `SELECT count(*) FROM projects` 符合筆數。

### 全部執行

- **行為**：`POST /api/runs` body `{ "trigger": "manual_all" }` 建立 `runs` 列，對所有 `projects` 插入 `run_projects`（queued），worker 依 `max_concurrency` 執行；每專案 `cwd=repo_path` 執行 `claude -p "<固定 prompt 觸發 reviewer-batch>"`；逾時 `per_project_timeout_sec` kill 子行程。
- **產出**：skill 寫入 `$DATA_ROOT_DIR/reports/<name>/<person>/<YYYY-MM-DD>/summary.md`；後端解析入 `reports`, `pending_items`。
- **去重**：同 `project_id` 已有 `run_projects.state IN ('queued','running')` 時 MUST 拒絕重複排入（HTTP 409）。
- **驗收**：mock 子行程或 fixture summary 檔；`GET /api/runs/:id` 顯示 `success|partial`；DB 有 report 列。

### 排程

- **行為**：`schedule_config.enabled=1` 時，依 `cadence`/`weekday`/`run_time` 觸發與 manual_all 相同流程，`runs.trigger='schedule'`。
- **驗收**：單元測試 mock clock 或手動改 config 觸發一次 run 列。

### 報告閱讀 API

- **行為**：
  - `GET /api/people` — 人員列表 + 未讀數 + open pending 數
  - `GET /api/people/:id/reports/latest` — 該人各專案最新一期 summary 渲染結果（sections: highlights, growth, pending, one_line）
  - `PATCH /api/reports/:id/read` — `is_read=1`
- **驗收**：fixture DB + summary 檔；API JSON schema 符合 contract。

### 前端 MVP

- **行為**：單頁 — 左欄人員、右側本週總覽（跨專案 one_line + 各專案卡片）；點人顯示詳情；「全部執行」按鈕；run 完成後 banner 通知。
- **驗收**：手動或 Playwright smoke：載入列表、標記已讀、觸發 run（可 stub 後端）。

**Scope 邊界**：本 change 不包含 MR 輪詢、`mr_reviews` 表寫入、趨勢讀檔 API。

## Risks / Trade-offs

- **[Risk] 伺服器無 claude CLI** → 文件化部署需求；執行失敗寫入 `run_projects.error`，不 crash 整批。
- **[Risk] skill 輸出格式漂移** → 解析器對固定 heading 契約校驗，缺段落記 warning 仍入庫 metadata。
- **[Risk] 無認證暴露 API** → MVP 僅內網 / VPN；文件註明 production 需 proxy auth。

## Migration Plan

1. 部署後端，設定 `DATA_ROOT_DIR`、準備 `projects.yaml` 與 git clone。
2. 確認 `/health`。
3. 手動 POST 全部執行驗證 pipeline。
4. 部署 frontend static 至同域或 CORS 允許來源。
5. 啟用排程。

Rollback：停止服務；DB 與 reports 目錄保留，不 destructive migrate down。

## Open Questions

- 前端框架 React vs Svelte（apply 第一個 frontend task 前定案，預設 React）。
- reviewer-batch skill prompt 字串（apply 時對齊既有 skill 名稱）。

