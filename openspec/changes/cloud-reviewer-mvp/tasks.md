## 1. 後端啟動與 Rust 後端框架選用 axum + sqlx + tokio（backend-foundation）

- [x] 1.1 【Requirement: Server initializes data directory and database】建立 `backend/`（axum + sqlx + tokio），未設定 `DATA_ROOT_DIR` 時非零 exit 且 stderr 含變數名 — 驗證：`cargo test startup_fails_without_data_dir`
- [x] 1.2 【Requirement: Server initializes data directory and database】migration 建立 MVP 表並啟用 foreign keys — 驗證：`cargo test migrations_apply_on_empty_db`
- [x] 1.3 【Requirement: Server initializes data directory and database】啟動時建立 `$DATA_ROOT_DIR/repos` 與 `reports` — 驗證：`cargo test data_dir_layout_created`
- [x] 1.4 【Requirement: Health endpoint reports readiness】實作 `GET /health` 回 `{ status: ok, data_dir }` — 驗證：`cargo test health_endpoint_returns_ok`

## 2. projects.yaml 載入（project-config）

- [x] 2.1 【Requirement: Projects load from YAML at startup】啟動讀取 `projects.yaml` 並 upsert `projects` — 驗證：`cargo test projects_yaml_loads_two_rows`
- [x] 2.2 【Requirement: Git repository detection updates project metadata】偵測 `repo_path` git 狀態寫入 `is_git_repo`/`default_branch` — 驗證：`cargo test git_detection_sets_default_branch`

## 3. reviewer-execution 執行引擎

- [x] 3.1 【Requirement: Manual batch run enqueues all projects】`POST /api/runs` manual_all 建立佇列；重複專案回 409 — 驗證：`cargo test manual_all_run_enqueues_projects` / `duplicate_project_run_returns_409`
- [x] 3.2 【Requirement: Worker executes reviewer skill subprocess per project】worker pool 執行 `claude -p`；逾時 kill 標 `skipped_timeout` — 驗證：`cargo test worker_marks_skipped_timeout`
- [x] 3.3 【Requirement: Summary files are parsed into reports and pending items】【summary.md 格式】【MVP 人員來源】解析 frontmatter 自動 upsert people、寫 reports/pending_items — 驗證：`cargo test summary_parser_creates_report_and_pending`

## 4. 排程實作（scheduling）

- [x] 4.1 【Requirement: Schedule configuration is stored as a single row】seed `schedule_config` 單列預設 — 驗證：`cargo test schedule_config_seeded`
- [x] 4.2 【Requirement: Enabled schedule triggers weekly batch runs】【排程實作】tokio-cron-scheduler 在 enabled=1 時觸發 schedule run — 驗證：`cargo test scheduled_run_creates_schedule_trigger`

## 5. 報告閱讀 API（report-reader）

- [x] 5.1 【Requirement: People list API exposes read and pending status】`GET /api/people` — 驗證：`cargo test people_list_includes_unread`
- [x] 5.2 【Requirement: Latest weekly report content is served per person】`GET /api/people/:id/reports/latest` — 驗證：`cargo test latest_reports_returns_sections`
- [x] 5.3 【Requirement: Reports can be marked read】`PATCH /api/reports/:id/read` — 驗證：`cargo test mark_report_read`
- [x] 5.4 `GET /api/runs/:id` 供前端輪詢 — 驗證：`cargo test get_run_by_id_returns_terminal_status`

## 6. 前端 MVP（report-reader）

- [x] 6.1 【前後端目錄結構】建立 `frontend/` Vite SPA 並連線 `/health` — 驗證：`npm run build`
- [x] 6.2 【Requirement: Web UI displays weekly reader and run controls】左欄人員 + 本週面板 + mark-read — 驗證：Playwright 或手動 smoke
- [x] 6.3 【Requirement: Web UI displays weekly reader and run controls】全部執行按鈕 + run 完成 banner — 驗證：手動 smoke POST /api/runs

## 7. 部署文件

- [x] 7.1 新增 `projects.yaml` 範例與 README（DATA_ROOT_DIR、claude CLI）— 驗證：依 README 啟動通過 `/health`


