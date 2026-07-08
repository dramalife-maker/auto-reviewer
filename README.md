# Auto Reviewer

雲端 1on1 Reviewer MVP：Rust 後端排程執行 reviewer-batch workflow，Vite 前端閱讀週報。

## 需求

- Rust（stable）與 Cargo
- Node.js 20+（前端 build / dev）
- AI agent CLI（擇一，見 `REVIEWER_AGENT`）：
  - [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code)（`claude` 已登入；設 `REVIEWER_AGENT=claude`）
  - [Cursor Agent CLI](https://cursor.com/docs/cli)（`cursor-agent` 已安裝；預設，須以已登入的使用者啟動 reviewer-server）
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
| `REVIEWER_AGENT` | 批次執行用的 agent：`cursor`（預設）或 `claude` |
| `REVIEWER_MODEL` | 可選的 model（對應 agent CLI 的 `--model`）；未設定則使用 agent 預設 |
| `REVIEWER_EXECUTOR` | 測試用 mock 執行檔，取代真實 agent CLI |
| `REVIEWER_MIN_FREE_BYTES` | clone / worktree add 前要求的最小可用空間，預設 2GiB |
| `CORS_ALLOW_ORIGINS` | 允許的前端來源（逗號分隔）。開發建置（`cargo run`）未設定時預設 `*`；正式建置未設定則不啟用 CORS。可明確設 `*` 或例 `https://reviewer.example.com` |

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

Vite dev server 會將 `/health` 與 `/api/*` proxy 到 repo 根目錄 `.env` 的 `PORT`（預設 `8080`）。本地開發請保持 `frontend/.env` 的 `VITE_API_BASE` 留空。

Production build：

```powershell
cd frontend
npm run build
```

靜態檔輸出至 `frontend/dist/`。可同域 proxy，或前後端分離部署。

#### 前後端分離部署（跨域）

前端 `https://reviewer.example.com`、後端 `https://api.example.com`：

```env
# 後端 .env
CORS_ALLOW_ORIGINS=https://reviewer.example.com

# frontend/.env.production
VITE_API_BASE=https://api.example.com
```

建置後前端會直接向後端 API 發請求。正式環境請設定 `CORS_ALLOW_ORIGINS` 為前端網址；開發建置（`cargo run`）未設定時預設允許所有來源（`*`）。

#### 子路徑部署（nginx，同域）

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

## 人員歸戶（git email）

同一工程師可能有多個 git email（同 repo 不同帳號、跨 GitLab/GitHub）。系統以 `git_email` identity 將 commit 歸到單一 `people` 列。**未綁定的 email 不會產出週報**，也不會自動建立人員。

### 建議流程（預先綁定）

1. `POST /api/people` 建立人員（`display_name` 即週報目錄名）
2. `POST /api/people/{id}/identities` 綁定已知 email（`kind: "git_email"`）
3. 執行 review（全部執行或排程）

### 事後指認（未歸戶佇列）

1. 先執行 review → 未綁定 email 進入 `unmatched_authors`
2. Web UI header「未歸戶」面板：建立新人員並綁定，或綁定到既有人員
3. 重新執行 review，該工程師才會出現在左欄並產出週報

`summary.md` frontmatter 的 `person` 必須等於 canonical `display_name`；後端 ingestion 不再自動 INSERT 新人員。

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

後端依 `REVIEWER_AGENT` 載入 workflow：

| `REVIEWER_AGENT` | 載入方式 |
|------------------|----------|
| `cursor`（預設） | 將兩檔內容內嵌至 prompt，並以 `cursor-agent --print --trust --force` 執行 |
| `claude` | `--append-system-prompt-file` 載入 `WORKFLOW.md` 與 `output-contract.md` |

動態參數僅 manifest 路徑（見 `docs/idea/spec.md` §6.0）。

執行前請確認：

1. 所選 agent CLI 已在 PATH 且已 auth（Cursor：以已執行 `cursor-agent login` 的同一使用者啟動 reviewer-server）
2. 從 repo 根目錄啟動後端（或設定 `APP_ROOT` 指向含 `skills/` 的目錄）
3. 各專案 `git_remote_url` 可達，且啟動時已成功 provision（`is_git_repo=1`）

本地測試 pipeline 時可設 `REVIEWER_EXECUTOR` 指向 mock script，無需真實 agent CLI。

## API 摘要

| 方法 | 路徑 | 說明 |
|------|------|------|
| GET | `/health` | 健康檢查 |
| POST | `/api/runs` | `{ "trigger": "manual_all" }` 全部執行 |
| GET | `/api/runs/{id}` | 查詢 run 狀態 |
| GET | `/api/people` | 人員列表（含未讀數、`identity_count`） |
| POST | `/api/people` | 建立人員 `{ "display_name": "..." }` |
| GET | `/api/people/{id}/identities` | 列出已綁 identity |
| POST | `/api/people/{id}/identities` | 綁定 identity `{ "kind", "value", "label?" }` |
| GET | `/api/unmatched-authors` | 未歸戶 git author 列表 |
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
