# Auto Reviewer

雲端 1on1 Reviewer MVP：Rust 後端排程執行 reviewer-batch workflow，Vite 前端閱讀週報。

## 需求

- Rust（stable）與 Cargo
- Node.js 20+（前端 build / dev）
- [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code)（`claude` 已登入；實際批次執行時需要）
- `git` 已在 PATH（後端啟動時會 bare clone 各專案並建立 worktree）

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
| `REVIEWER_MIN_FREE_BYTES` | clone / worktree add 前要求的最小可用空間，預設 2GiB |

### 2. 專案設定

複製範本後編輯（`projects.yaml` 已被 git 忽略，不會提交）：

```powershell
copy projects.yaml.example projects.yaml
```

每個 project 需填 `git_remote_url`（必填）與 `default_branches`（必填、非空）：

```yaml
projects:
  - name: game-backend
    repo_path: game-backend                 # → $DATA_ROOT_DIR/repos/game-backend
    git_remote_url: git@gitlab.example.com:team/game-backend.git
    default_branches:
      - main
  - name: web-portal
    repo_path: test/web-portal
    git_remote_url: git@gitlab.example.com:team/web-portal.git
    default_branches:
      - main
      - develop
```

`repo_path` 是一個 **bare+worktree 容器目錄**，不是已 checkout 的工作副本。後端啟動時會在其中建立：

```text
$DATA_ROOT_DIR/repos/<repo_path>/
  .bare/            # git clone --bare
  .git              # 檔案，內容 gitdir: ./.bare
  main/             # 常駐 worktree（每個 default_branches 一個）
  <mr-branch>/      # MR review 時按需建立
```

`repo_path` 解析規則：

| `repo_path` 寫法 | 實際目錄 |
|------------------|----------|
| `game-backend` | `$DATA_ROOT_DIR/repos/game-backend` |
| `test/projectA` | `$DATA_ROOT_DIR/repos/test/projectA` |
| `/srv/git/foo` | 絕對路徑，不變 |
| `./custom/path` | 相對 process cwd，不變（相容舊寫法） |

無需手動 clone；後端會 provision。若 project 缺 `git_remote_url`、`default_branches` 為空、clone 失敗或磁碟不足，該 project 會被標記 **unhealthy**（`is_git_repo=0`、記錄原因）並隔離，不影響其他 project、也不會使啟動失敗。

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

#### 子路徑部署（nginx）

若前端掛在子路徑（例 `https://example.com/reviewer/`），編譯前設定：

```powershell
# frontend/.env.production
VITE_BASE_PATH=/reviewer
```

```powershell
cd frontend
npm run build
```

建置產物中的 JS/CSS 會帶 `/reviewer/` 前綴。nginx 範例：

```nginx
location /reviewer/ {
    alias /var/www/reviewer/dist/;
    try_files $uri $uri/ /reviewer/index.html;
}

# API 若在網域根（常見）
location /api/ {
    proxy_pass http://127.0.0.1:8080;
}
location = /health {
    proxy_pass http://127.0.0.1:8080/health;
}
```

若 API 也掛在同一前綴下（`/reviewer/api/`），額外設定：

```env
VITE_API_BASE=/reviewer
```

並在 nginx 將 `/reviewer/api/` proxy 到後端。詳見 `frontend/.env.example`。

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
3. 各專案 `git_remote_url` 可達，且啟動時已成功 provision（`is_git_repo=1`）

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

排程預設：每週一 09:00 台北時間（`schedule_config` 表，enabled=1）。時區由 `schedule_config.tz_offset_min` 設定（UTC 偏移分鐘數，預設 `480` = UTC+8）；`run_time` 依此時區解讀。

## 測試

```powershell
cargo test -p reviewer-server
cd frontend && npm run build
```

## 文件

- 產品規格：`docs/idea/spec.md`
- 資料 schema：`docs/idea/schema.md`
- MVP 實作計畫：`openspec/changes/cloud-reviewer-mvp/`
