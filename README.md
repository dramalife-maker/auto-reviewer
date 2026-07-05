# Auto Reviewer

雲端 1on1 Reviewer MVP：Rust 後端排程執行 reviewer-batch workflow，Vite 前端閱讀週報。

## 需求

- Rust（stable）與 Cargo
- Node.js 20+（前端 build / dev）
- [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code)（`claude` 已登入；實際批次執行時需要）
- 各專案 git clone 已放在 `DATA_ROOT_DIR/repos/<project-name>/`

## 快速開始

### 1. 環境變數

```powershell
copy .env.example .env
```

`.env` 至少設定：

| 變數 | 說明 |
|------|------|
| `DATA_ROOT_DIR` | 資料根目錄（SQLite、`repos/`、`reports/`） |
| `PORT` | HTTP 埠，預設 `8080` |

可選：

| 變數 | 說明 |
|------|------|
| `PROJECTS_CONFIG` | `projects.yaml` 路徑，預設 repo 根目錄 |
| `APP_ROOT` | 部署根目錄，預設為 process 工作目錄；headless workflow 在 `$APP_ROOT/skills/` |
| `REVIEWER_EXECUTOR` | 測試用 mock 執行檔，取代 `claude` |

### 2. 專案設定

編輯 repo 根目錄的 `projects.yaml`，將 `repo_path` 指向 `$DATA_ROOT_DIR/repos/` 下的 clone：

```yaml
projects:
  - name: game-backend
    repo_path: ./data/reviewer/repos/game-backend
    git_remote_url: git@gitlab.example.com:team/game-backend.git
  - name: web-portal
    repo_path: ./data/reviewer/repos/web-portal
```

`repo_path` 可為相對路徑（相對於 process 工作目錄）或絕對路徑。

### 3. 啟動後端

在 repo 根目錄：

```powershell
cargo run -p reviewer-server
```

驗證：

```powershell
curl http://127.0.0.1:8080/health
```

預期回應：`{"status":"ok","data_dir":"..."}`

首次啟動會建立 `$DATA_ROOT_DIR/reviewer.db`、執行 migration，並建立 `repos/`、`reports/` 目錄。

### 4. 啟動前端（開發）

```powershell
cd frontend
npm install
npm run dev
```

Vite dev server 會將 `/health` 與 `/api/*` proxy 到 `http://127.0.0.1:8080`。

Production build：

```powershell
cd frontend
npm run build
```

靜態檔輸出至 `frontend/dist/`，可交由 reverse proxy 與後端同域部署。

## Headless 執行

Worker 對每個專案 spawn：

```text
claude --bare ... --append-system-prompt-file $APP_ROOT/skills/reviewer-batch/WORKFLOW.md ...
```

### Bundled workflow（`skills/reviewer-batch/`）

| 檔案 | 用途 |
|------|------|
| `WORKFLOW.md` | 週報 headless 流程（讀 manifest → git 分析 → 寫 report/summary → 更新長期檔） |
| `output-contract.md` | `summary.md` 格式契約（frontmatter + 三個固定 heading） |

後端 spawn 時以 `--append-system-prompt-file` 載入上述兩檔；動態參數僅 manifest 路徑（見 `docs/idea/spec.md` §6.0）。

執行前請確認：

1. `claude` 已在 PATH 且已 auth
2. 從 repo 根目錄啟動後端（或設定 `APP_ROOT` 指向含 `skills/` 的目錄）
3. 各專案 `repo_path` 為有效 git 目錄

本地測試 pipeline 時可設 `REVIEWER_EXECUTOR` 指向 mock script，無需真實 `claude`。

## API 摘要

| 方法 | 路徑 | 說明 |
|------|------|------|
| GET | `/health` | 健康檢查 |
| POST | `/api/runs` | `{ "trigger": "manual_all" }` 全部執行 |
| GET | `/api/runs/{id}` | 查詢 run 狀態 |
| GET | `/api/people` | 人員列表（含未讀數） |
| GET | `/api/people/{id}/reports/latest` | 最新週報 |
| PATCH | `/api/reports/{id}/read` | 標記已讀 |

排程預設：每週一 09:00（`schedule_config` 表，enabled=1）。

## 測試

```powershell
cargo test -p reviewer-server
cd frontend && npm run build
```

## 文件

- 產品規格：`docs/idea/spec.md`
- 資料 schema：`docs/idea/schema.md`
- MVP 實作計畫：`openspec/changes/cloud-reviewer-mvp/`
