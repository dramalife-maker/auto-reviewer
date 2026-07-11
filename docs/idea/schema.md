# Reviewer 雲端 Web 服務 — 資料 Schema 附錄

> 搭配《設計規格》（spec.md）使用。路徑與檔名約定與 spec **§9.0** 一致；UI 欄位名 = 本附錄 SQL 欄位名。
> 決定（拍板）：
> - **部署**：Rust 後端常駐於雲端主機；SQLite 存**設定 + 執行狀態 + 本週操作狀態**（非趨勢長期資料）。
> - **報告檔**：`report.md`（完整）與 `summary.md`（精簡）留在檔案系統，DB 僅存路徑與 metadata。
> - **趨勢**：**讀檔**——`index.md`、`YYYY-MM.md`、`_notes.md` 等既有 1on1 結構；**不在 SQLite 建 trend 表**。

---

## 0. 儲存佈局

```
$DATA_ROOT_DIR/                   # 環境變數，例 /data/reviewer
├── reviewer.db                      # SQLite（設定、狀態、收件匣）
├── repos/                           # projects.repo_path 指向此下
│   └── <slug>/
├── runs/                            # headless manifest（spec §6.0）
│   └── <run_id>/projects/<project_id>/manifest.json
└── reports/
    ├── _people/                     # 人物層（跨專案長期觀察；非專案名）
    │   └── <person>/                # = people.display_name
    │       ├── index.md             # 趨勢「長期觀察」（跨專案）
    │       ├── YYYY-MM.md           # 趨勢「成長軌跡」（跨專案月度綜合）
    │       └── _notes.md            # 趨勢「歷史待確認」
    └── <name>/                      # 通常 = projects.name
        └── <person>/                # 通常 = people.display_name
            ├── index.md             # （可選）本專案技術脈絡
            ├── YYYY-MM.md           # 本專案月度成長素材（workflow 每週追加）
            ├── _pending/            # 軌道 2 MR 觀察片段
            ├── _cache/              # 選填：long_term.md 等 AI 快取（spec §8 #4）
            └── <YYYY-MM-DD>/
                ├── report.md
                └── summary.md       # 本週 Tab（§7 格式）
```

- **`DATA_ROOT_DIR`**：後端環境變數（spec §0、§9.0）。
- **`APP_ROOT`**：部署根目錄（app repo）；headless workflow 位於 `$APP_ROOT/skills/`（spec §6.0、§9.4）。
- **`repo_path`** 慣例：`$DATA_ROOT_DIR/repos/<slug>/`。
- 週報與 1on1 長期檔以檔案為準；DB 以路徑引用週報。趨勢 API 讀檔，不查 trend 表。

---

## 1. ER 概觀

```
people ──< person_identities
people ──< participation >── projects        （由 identity 比對推導，物化於此表）
projects ──< reports >── people
reports ──< pending_items                     （本週待確認 + 操作閉環；非趨勢主資料源）
runs ──< run_projects >── projects
projects ──< mr_reviews >── people            （軌道 2：MR review 收件匣）
（單列）schedule_config
```

> **無** `trend_snapshots` / `growth_milestones` 表。趨勢 Tab 長期資料讀檔（spec §2.6、§9）；`pending_items` 僅驅動本週待確認與 web 介面上的手動閉環操作。

---

## 2. 核心設定表

### 2.1 `people` — 工程師

```sql
CREATE TABLE people (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    display_name TEXT    NOT NULL,           -- 1on1 / 報告顯示名，例 "Alice"
    avatar_seed  TEXT,                       -- 頭像底色 / 字母來源，可為 NULL（用 display_name 首字）
    created_at   TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT    NOT NULL DEFAULT (datetime('now'))
);
```

### 2.2 `person_identities` — git author 歸戶依據（一人多筆）

```sql
CREATE TABLE person_identities (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    person_id   INTEGER NOT NULL REFERENCES people(id) ON DELETE CASCADE,
    kind        TEXT    NOT NULL,            -- 'git_email' | 'gitlab_user' | 'glab_user'
    value       TEXT    NOT NULL,            -- 例 "alice@team.io" 或 "alice_w"
    label       TEXT,                        -- 顯示標籤："公司" | "個人" | "glab user"
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    -- 衝突驗證：同一 identity 不可綁到兩個人（待決 #6 嚴格模式）
    UNIQUE (kind, value)
);

CREATE INDEX idx_identities_person ON person_identities(person_id);
CREATE INDEX idx_identities_lookup ON person_identities(kind, value);
```

> 歸戶：執行時取 commit author email / glab MR author，以 `(kind, value)` 命中 `person_identities` → 得 `person_id`。命中不到的 author 進入 `unmatched_authors`（見 2.6）。
> `git_email` 會 trim + 小寫；`gitlab_user` / `glab_user` 只 trim、不強制小寫。同人重複 bind 為 no-op；跨人衝突回 409。

#### People settings API

| 方法 | 路徑 | 說明 |
|------|------|------|
| GET | `/api/people/{id}` | 詳情：`display_name`、`identities`、`projects`（`reports` ∪ `participation` 去重專案名） |
| PATCH | `/api/people/{id}` | 更名 `{ "display_name" }`；同步 rename `reports/_people/{old}/` → `{new}/` |
| DELETE | `/api/people/{id}/identities/{identity_id}` | 解除綁定（允許刪到零）；錯人／不存在回 404 |

**更名限制**：只更新 DB 與人物層 `_people/` 目錄。不搬專案層 `reports/{project}/{display_name}/`，不改歷史 summary frontmatter。目標目錄已存在或顯示名重名 → 409；目錄 rename 失敗則回滾 DB 顯示名。

### 2.3 `projects` — 專案

```sql
CREATE TABLE projects (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    name           TEXT    NOT NULL UNIQUE,  -- 顯示名 / Tab 名；reports/<name>/ 目錄名
    repo_path      TEXT    NOT NULL,         -- 伺服器 git working copy；慣例 $DATA_ROOT_DIR/repos/<slug>/
    git_remote_url TEXT,                     -- 選填；spec §8 #7 自動 clone / pull
    default_branch TEXT,                     -- 偵測得出，例 "main"
    is_git_repo    INTEGER NOT NULL DEFAULT 0, -- 路徑偵測結果（0/1）
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);
```

> `repo_path` 為 headless 子行程 `cwd`；`glab` 於此目錄執行（spec §6.0）。

### 2.4 `participation` — 參與關係（物化）

由 identity 比對推導，但物化成表以加速「專案的參與工程師」「人的參與專案」查詢。每次執行後重算更新。

```sql
CREATE TABLE participation (
    project_id   INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    person_id    INTEGER NOT NULL REFERENCES people(id)   ON DELETE CASCADE,
    last_seen_at TEXT,                       -- 最後一次在此專案偵測到 commit 的時間
    PRIMARY KEY (project_id, person_id)
);
```

### 2.5 `schedule_config` — 排程設定（單列）

```sql
CREATE TABLE schedule_config (
    id              INTEGER PRIMARY KEY CHECK (id = 1),  -- 強制單列
    enabled         INTEGER NOT NULL DEFAULT 1,
    cadence         TEXT    NOT NULL DEFAULT 'weekly',   -- 'daily' | 'weekly' | 'biweekly'
    weekday         INTEGER,                              -- 0=週一 ... 6=週日（daily 時為 NULL）
    run_time        TEXT    NOT NULL DEFAULT '09:00',     -- HH:MM，24h（依 tz_offset_min 時區解讀）
    tz_offset_min   INTEGER NOT NULL DEFAULT 480,         -- run_time 時區，UTC 偏移分鐘，480=UTC+8 台北
    mr_poll_interval_min INTEGER NOT NULL DEFAULT 60,    -- 軌道 2 MR 輪詢間隔（分鐘）
    per_project_timeout_sec INTEGER NOT NULL DEFAULT 600, -- 單專案逾時，預設 10 分鐘
    max_concurrency INTEGER NOT NULL DEFAULT 2,           -- worker pool 並發上限
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO schedule_config (id) VALUES (1);  -- 初始化單列
```

> 排程由後端內建 `tokio-cron-scheduler` 驅動（spec §7.3 已決），非 OS cron、非桌面常駐。
>
> **API：** `GET`／`PATCH /api/schedule` 讀寫上表；`PATCH` 可更新 `enabled`、`weekday`、`run_time`、`tz_offset_min`、`per_project_timeout_sec`、`max_concurrency`、`mr_poll_interval_min`；`cadence` 本輪僅允許 `weekly`（其他值 → 400）。影響 cron 的欄位需重啟 `reviewer-server`；`per_project_timeout_sec`／`max_concurrency` 下一場 run 即生效。
>
> **漏跑：** `enabled=1` 時推算最近 `due_at`；無覆蓋的 `schedule`／`manual_all` run（`started_at >= due_at - 6h`，狀態 success／partial／running／queued）則回傳 `missed_weekly_run`。`POST /api/schedule/catch-up` 建立等同 `manual_all` 的補跑（衝突 409）。MR 輪詢不做漏跑補償。

### 2.6 `unmatched_authors` — 未歸戶 author（待人工指認）

```sql
CREATE TABLE unmatched_authors (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    kind        TEXT    NOT NULL,            -- 'email' | 'glab_user'
    value       TEXT    NOT NULL,
    project_id  INTEGER REFERENCES projects(id) ON DELETE SET NULL,
    first_seen  TEXT    NOT NULL DEFAULT (datetime('now')),
    last_seen   TEXT    NOT NULL DEFAULT (datetime('now')),
    commit_count INTEGER NOT NULL DEFAULT 1,
    UNIQUE (kind, value)
);
```

> 用途：執行時遇到無法歸戶的 author，記在此。人員設定頁可提示「N 個未歸戶 author」，讓管理者把 identity 補綁到對應的人，補完即從本表移除。

---

## 3. 報告表

### 3.1 `reports` — 每位工程師每專案每次執行一份

```sql
CREATE TABLE reports (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id     INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    person_id      INTEGER NOT NULL REFERENCES people(id)   ON DELETE CASCADE,
    run_id         INTEGER REFERENCES runs(id) ON DELETE SET NULL,
    report_date    TEXT    NOT NULL,         -- YYYY-MM-DD，該批執行日期
    report_md_path TEXT    NOT NULL,         -- $DATA_ROOT_DIR/reports/<name>/<person>/<date>/report.md
    summary_md_path TEXT   NOT NULL,         -- 同目錄 summary.md
    one_line       TEXT,                     -- 總覽用「一句話摘要」（冗餘存 DB 便於列表查詢）
    mr_count       INTEGER,                  -- 該期 MR 數（僅顯示「6 MR」用，非趨勢指標）
    commit_count   INTEGER,                  -- 同上
    is_read        INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE (project_id, person_id, report_date)
);

CREATE INDEX idx_reports_person_date ON reports(person_id, report_date DESC);
CREATE INDEX idx_reports_unread ON reports(is_read) WHERE is_read = 0;
```

> 註：`mr_count` / `commit_count` 僅供單一專案報告頂部顯示概況（「6 MR」），**不**用於趨勢頁（趨勢無數量圖，見規格 §0 / §2.6）。

### 3.2 `pending_items` — 待確認項目（本週操作 + 閉環狀態）

```sql
CREATE TABLE pending_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    person_id   INTEGER NOT NULL REFERENCES people(id)    ON DELETE CASCADE,
    project_id  INTEGER NOT NULL REFERENCES projects(id)  ON DELETE CASCADE,
    report_id   INTEGER REFERENCES reports(id) ON DELETE SET NULL, -- 提出此項的報告
    question    TEXT    NOT NULL,            -- 開放式問句
    status      TEXT    NOT NULL DEFAULT 'open',  -- 'open' | 'resolved'
    raised_date TEXT    NOT NULL,            -- YYYY-MM，提出月份
    resolved_date TEXT,                      -- YYYY-MM，標記改善的月份
    resolution_note TEXT,                    -- 選填：如何閉環的簡述
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_pending_person_status ON pending_items(person_id, status);
```

> **分工**：
> - **本週 Tab / 左欄 ⚠️ 標記**：查 `pending_items WHERE status='open'`。
> - **管理者手動閉環**：透過 Closure API `PATCH /api/pending-items/{id}` 標 `resolved`（DB-first；成功後同步改寫該人物的 `_notes.md`，見下方 B1 格式）。
> - **趨勢 Tab「歷史待確認」**：**主資料源為檔案**（`_notes.md`、各月 `YYYY-MM.md` / `summary.md` 的待確認區段）；DB 列為操作輔助，非 trend 主存儲。

#### Closure API

| 方法 | 路徑 | 說明 |
|------|------|------|
| GET | `/api/people/{id}/pending-items?status=open\|resolved\|all` | 列出某人待確認項目；預設 `open` |
| PATCH | `/api/pending-items/{id}` | `{ "status": "resolved", "resolution_note"?: string }`，僅允許 `open → resolved` |

`PATCH` 成功會將 `resolved_date` 設為依 `schedule_config.tz_offset_min` 計算的當月 `YYYY-MM`，並同步改寫人物層 `_notes.md`（DB 更新成功、檔案寫入失敗時回 HTTP 502，DB 仍保持 `resolved`）。已 `resolved` 項目再次 `PATCH` 回 409；`status` 非 `resolved` 回 400；未知 `id` 回 404。

#### `_notes.md` B1 行格式

- open：`- [YYYY-MM] {question}`
- resolved：`- [YYYY-MM→YYYY-MM] ✓ {question}`，若有 `resolution_note` 則行尾加 ` — {note}`

範例：

```text
- [2026-07] Why choose A?
- [2026-06→2026-07] ✓ Earlier concern — fixed in review
```

閉環時會尋找第一筆文字完全相符的 open 行並改寫；找不到則 append 一筆 resolved 行；檔案或目錄不存在則建立後寫入。

> **BREAKING**：`GET /api/people/:id/reports/latest` 各專案卡的欄位由 `pending: string[]` 改為 `pending_items`（物件陣列，含 `id`/`question`/`status` 等，僅含該專案 `status='open'` 列）。`GET /api/people/:id/trends` 的 `historical_pending` 由 `string[]` 改為結構化物件陣列（`question`/`status`/`raised_month`/`resolved_month`/`resolution_note`/`raw_line`）。

---

## 4. 執行紀錄表

### 4.1 `runs` — 每次批次 / 單專案執行

```sql
CREATE TABLE runs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger     TEXT    NOT NULL,            -- 'schedule' | 'manual_all' | 'manual_project' | 'mr_poll' | 'manual_mr_poll'
    status      TEXT    NOT NULL DEFAULT 'running', -- 'running' | 'success' | 'partial' | 'failed'
    started_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT,
    duration_sec INTEGER,
    project_total INTEGER,                   -- 本次涵蓋專案數
    project_skipped INTEGER NOT NULL DEFAULT 0, -- 逾時跳過數
    note        TEXT
);

CREATE INDEX idx_runs_started ON runs(started_at DESC);
CREATE INDEX idx_runs_status_started ON runs(status, started_at DESC);
CREATE INDEX idx_runs_trigger_started ON runs(trigger, started_at DESC);
```

> `status='partial'`：有專案逾時跳過（規格 §7.2）。`trigger='mr_poll'`／`manual_mr_poll` 供軌道 2 輪詢／手動掃描紀錄。

### 4.2 `run_projects` — 單次執行內各專案明細

```sql
CREATE TABLE run_projects (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id      INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    state       TEXT    NOT NULL DEFAULT 'queued', -- 'queued'|'running'|'done'|'skipped_timeout'|'failed'
    started_at  TEXT,
    finished_at TEXT,
    duration_sec INTEGER,
    error       TEXT,
    UNIQUE (run_id, project_id)
);

CREATE INDEX idx_run_projects_run ON run_projects(run_id);
```

> 執行面板 / 專案清單列的即時狀態由本表驅動（佇列中 / 分析中 / 完成 / 逾時跳過）。
> 去重鎖（規格 §6.3）：插入前檢查同 `project_id` 是否已有 `state IN ('queued','running')` 的列。

### 4.2.1 執行紀錄 API（Web 可觀測）

| 方法 | 路徑 | 說明 |
|------|------|------|
| GET | `/api/runs` | `{ "runs": [...], "total": N }`；每筆：`id`、`trigger`、`status`、`started_at`、`finished_at`、`duration_sec`、`project_total`、`project_skipped`。Query：`limit`（預設 50、最大 200）、`offset`、`trigger`、`status`。非法 limit／offset → 400。 |
| GET | `/api/runs/{id}` | 明細：run 級 `duration_sec`／`note`；各專案 `name`／`state`／`error`／`started_at`／`finished_at`／`duration_sec`。未知 id → 404。 |
| GET | `/api/dashboard` | 另含 `recent_runs`：最多 5 筆 list item，依 `started_at` 降序。 |

**MR skip 摘要**（僅 `trigger` 為 `mr_poll`／`manual_mr_poll`）：各專案物件附 `skip_summary`：

```json
{
  "by_reason": { "inbox_draft": 1, "gitlab_draft": 1 },
  "items": [
    { "mr_iid": 12, "skip_reason": "inbox_draft" },
    { "mr_iid": 8, "skip_reason": "gitlab_draft" }
  ]
}
```

來源為檔案（**不建 skip DB 表**）：

```
$DATA_ROOT_DIR/runs/<run_id>/projects/<project_id>/eligible_mrs.json
```

讀取該檔 `skipped[]` 彙總；`items` 最多 100 筆（`by_reason` 仍為完整計數）。缺檔或無法讀取 → 空摘要（`by_reason: {}`、`items: []`），整份明細仍 200。非 MR trigger、或 run 仍為 `status='running'`（polling 熱路徑）不附 `skip_summary`（省略欄位）。已結束的 MR run 在讀檔時走 blocking 執行緒，避免卡住 async runtime。

### 4.3 `manifest.json` — headless 執行契約（檔案，非 SQLite）

後端在 spawn Claude 子行程**前**寫入；Claude 以 Read 讀取，作為本次 run 的唯一動態參數（spec §6.0）。

**路徑**：

```
$DATA_ROOT_DIR/runs/<run_id>/projects/<project_id>/manifest.json
```

**週報（`mode: weekly_batch`）**：

```json
{
  "mode": "weekly_batch",
  "project_name": "game-backend",
  "repo_path": "/data/reviewer/repos/game-backend",
  "report_root": "/data/reviewer/reports/game-backend",
  "person_report_root": "/data/reviewer/reports/_people",
  "run_date": "2026-07-05",
  "since": "2026-06-28",
  "output_contract": "output-contract.md",
  "authors": [
    {
      "email": "alice@co.com",
      "git_name": "Alice",
      "person_id": 1,
      "display_name": "Alice Chen"
    }
  ],
  "open_pending": [
    {
      "id": 7,
      "person_id": 1,
      "display_name": "Alice Chen",
      "question": "Why choose A?"
    }
  ],
  "published_pending_snippets": []
}
```

| 欄位 | 說明 |
|------|------|
| `authors` | 本窗口已歸戶工程師；workflow 僅為這些人產報 |
| `open_pending` | 本專案目前 `pending_items.status='open'`；元素含 `id` / `person_id` / `display_name` / `question`。workflow 延續議題必須原句沿用；不再相關可省略（不自動 resolve） |
| `published_pending_snippets` | 可折入週報的已發佈 MR 觀察片段路徑（相對 `report_root`） |

**MR 輪詢（`mode: mr_poll`）** 另含：

| 欄位 | 說明 |
|------|------|
| `draft_dir` | MR 草稿落檔目錄（例 `runs/<run_id>/projects/<id>/drafts/`） |
| `pending_dir` | 觀察片段根目錄（例 `reports/<name>/<person>/_pending/`） |
| `reviewer_username` | GitLab 視角（例 `alice_w`） |
| `since` | 可選；輪詢去重窗口 |

**spawn 組裝**（後端實作；詳見 spec §6.0）：

- `--append-system-prompt-file` → `$APP_ROOT/skills/<workflow>/WORKFLOW.md`（＋ `output-contract.md`）
- `-p` → 短路徑，含 manifest 絕對路徑
- `cwd` = `manifest.repo_path`
- `--permission-mode dontAsk`、`--add-dir` 含 `DATA_ROOT_DIR` 與 `repo_path`

---

## 5. MR review 表（軌道 2）

### 5.1 `mr_reviews` — 收件匣草稿

headless workflow 產草稿檔，後端解析入庫；發佈時由後端代跑 `glab mr note`（人工按鈕觸發）。

```sql
CREATE TABLE mr_reviews (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id      INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    person_id       INTEGER REFERENCES people(id) ON DELETE SET NULL,  -- MR author（歸戶後）
    mr_iid          INTEGER NOT NULL,          -- GitLab MR !number / iid
    mr_title        TEXT,
    review_round    INTEGER NOT NULL DEFAULT 1, -- 1 | 2（scan-mrs 分輪；spec §6.4）
    draft_md_path   TEXT    NOT NULL,          -- 伺服器草稿檔；解析後路徑可冗餘存此
    status          TEXT    NOT NULL DEFAULT 'draft', -- 'draft' | 'published' | 'ignored'
    published_at    TEXT,
    published_body  TEXT,                      -- 實際發佈到 GitLab 的內容（可與草稿不同）
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE (project_id, mr_iid, review_round)
);

CREATE INDEX idx_mr_reviews_inbox ON mr_reviews(status, created_at DESC)
    WHERE status = 'draft';
```

> 觀察片段寫入 `reports/<name>/<person>/_pending/`（§0），非本表。僅**發佈後**納入週報（spec §6.5，待確認）。

---

## 6. 趨勢資料（讀檔，不入 SQLite）

| 趨勢 Tab 區塊 | 檔案來源 | 後端行為 |
|--------------|---------|---------|
| 長期觀察 | `reports/_people/<person>/index.md` | 讀檔全文；可接受無 frontmatter 的自由 Markdown |
| 成長軌跡 | `reports/_people/<person>/YYYY-MM.md` | 列舉月檔、依檔名降序回傳 JSON（跨專案綜合） |
| 歷史待確認 | `reports/_people/<person>/_notes.md` | 解析 `- [YYYY-MM]` 開頭的行 |

**專案層月檔**（趨勢 API 不讀；workflow 維護）：`reports/<project>/<person>/YYYY-MM.md` 記錄該人在該 repo 的月度成長，作為單專案深讀素材，並供 workflow 撰寫人物層月檔時參考。

**可選快取（不落 SQLite）**：AI 重算的「長期觀察」可寫入 `reports/<name>/<person>/_cache/long_term.md`（spec §8 #4）。

**headless workflow 責任**：每週產報後維護**專案層** `{project}/{person}/YYYY-MM.md`（本 repo 月度成長）與**人物層** `_people/<person>/` 下 `index.md`、`YYYY-MM.md`、`_notes.md`（跨專案綜合）。趨勢 API 只讀人物層，不寫 SQLite。

---

## 7. summary.md 格式（workflow 輸出 → API 渲染）

傾向（待決 #3）：**獨立 `summary.md`**。`reviewer-batch` workflow 對每位工程師每專案輸出；後端用 `pulldown-cmark` 轉 HTML / JSON 供前端。

格式約定（供 workflow 產出與後端解析一致）：

```markdown
---
person: Alice
project: game-backend
date: 2026-06-14
one_line: 本週主軸在資料庫效能與 CI 改善，整體穩定，有 1 項架構決策待確認。
mr_count: 6
commit_count: 42
---

## 本週重點
- 主導 `transaction_rounds` 分區索引重構，查詢成本顯著下降
- MR review 回應速度快，程式碼可讀性佳

## 成長面向
- 大型 PR 拆分顆粒度可再細，利於 review

## 待確認
- MR #234 架構選擇是主動決策還是時間壓力妥協？
- 分區索引上線後是否觀察過實際查詢分佈？

## 已釐清
```

解析規則：
- **frontmatter**（`---` 區塊）：抽 `person` / `project` / `date` / `one_line` / `mr_count` / `commit_count` → 寫入 `reports` 對應欄位。
- **`## 待確認`** 下每個 `-` 項 → 一筆 `pending_items`（`raised_date` = frontmatter `date` 的 YYYY-MM）；workflow 亦應寫入月檔 / `_notes.md` 供趨勢讀檔。
- **`## 已釐清`** 下每個 `-` 項：若存在同人同專案且 `question` 完全相同的 open `pending_items`，ingest **自動 resolve**（`resolved_date` = schedule 時區月份；同步 `_notes.md` resolved 行）。無匹配則忽略。僅省略待確認、未列於此區 → **不** resolve。
- **`## 本週重點` / `## 成長面向`**：API 直接回傳 md 或渲染結果，不需入庫（內容已在 summary.md）。
- heading 名稱為固定契約（四區：本週重點／成長面向／待確認／已釐清），workflow 與後端都依賴；變更需同步雙方。

> `report.md`（完整版）無格式約束，純供深讀；前端以「完整 md」連結由 API 提供 raw 內容。

---

## 8. 寫入流程（一次週報執行的資料動線）

```
1. runs：插入一列（trigger, status='running'）
2. 對每個納入的專案：
   a. run_projects：插入（state='queued'）→ 去重鎖檢查
   b. 取得鎖 → state='running'
   c. 寫 manifest.json → runs/<run_id>/projects/<project_id>/（§4.3）
   d. spawn `claude` headless（spec §6.0）：
      - cwd = projects.repo_path
      - --append-system-prompt-file → $APP_ROOT/skills/reviewer-batch/...
      - -p 含 manifest 路徑
      → 產出 reports/<name>/<person>/<date>/{report.md, summary.md}
      → 更新 index.md / YYYY-MM.md 等（workflow 寫檔；§0）
   e. 解析每份 summary.md frontmatter + 區段：
      - reports：upsert（UNIQUE project_id+person_id+report_date）
      - pending_items：新增 open 項（沿用未閉環的舊項則不重複）
      - `## 已釐清`：匹配 open 項 → resolve + 同步 `_notes.md`（notes 失敗不中止批次）
      - 歸戶：author 命中 person_identities → person_id；未命中 → unmatched_authors
   f. 重算 participation（該專案出現的 person_id 集合）
   g. state='done' / 'skipped_timeout'（逾時）/ 'failed'
3. runs：status='success'|'partial'|'failed'，補 finished_at / duration_sec / project_skipped
```

**軌道 2（MR 輪詢）**：manifest `mode=mr_poll`；載入 `skills/scan-mrs-headless/`；解析草稿 → `mr_reviews`；觀察片段 → `_pending/`。

---

## 9. 常用查詢（驅動各畫面）

```sql
-- 左欄工程師清單（含未讀、待確認標記、參與專案數）
SELECT p.id, p.display_name,
       COUNT(DISTINCT pt.project_id) AS project_count,
       SUM(CASE WHEN r.is_read = 0 THEN 1 ELSE 0 END) AS unread_count,
       (SELECT COUNT(*) FROM pending_items pi
          WHERE pi.person_id = p.id AND pi.status = 'open') AS open_pending
FROM people p
LEFT JOIN participation pt ON pt.person_id = p.id
LEFT JOIN reports r ON r.person_id = p.id
GROUP BY p.id;

-- 某人最近一期、跨專案的報告（總覽 Tab）
SELECT r.*, pr.name AS project_name
FROM reports r JOIN projects pr ON pr.id = r.project_id
WHERE r.person_id = :person_id
  AND r.report_date = (SELECT MAX(report_date) FROM reports WHERE person_id = :person_id)
ORDER BY pr.name;

-- 本週開放中的待確認（本週 Tab / 左欄標記）
SELECT pi.*, pr.name AS project_name
FROM pending_items pi
JOIN projects pr ON pr.id = pi.project_id
WHERE pi.person_id = :person_id AND pi.status = 'open'
ORDER BY pr.name, pi.raised_date DESC;

-- MR 收件匣（草稿）
SELECT mr.*, pr.name AS project_name, p.display_name AS author_name
FROM mr_reviews mr
JOIN projects pr ON pr.id = mr.project_id
LEFT JOIN people p ON p.id = mr.person_id
WHERE mr.status = 'draft'
ORDER BY mr.created_at DESC;

-- 控制台統計卡
SELECT
  (SELECT COUNT(*) FROM projects) AS projects,
  (SELECT COUNT(*) FROM people)   AS people,
  (SELECT COUNT(*) FROM reports WHERE is_read = 0) AS unread,
  (SELECT COUNT(*) FROM pending_items WHERE status = 'open') AS pending,
  (SELECT COUNT(*) FROM mr_reviews WHERE status = 'draft') AS mr_drafts;

-- 漏跑補償檢查（dashboard／GET schedule）
-- 推算最近 due_at；查詢是否有覆蓋 run：
SELECT COUNT(*) FROM runs
WHERE trigger IN ('schedule', 'manual_all')
  AND status IN ('success', 'partial', 'running', 'queued')
  AND started_at >= ?;  -- due_at - 6 hours (UTC)
-- 無覆蓋 → missed_weekly_run；POST /api/schedule/catch-up 手動補跑
```

**趨勢 Tab**：無固定 SQL；後端讀 `$DATA_ROOT_DIR/reports/<name>/<person>/` 下檔案組 API（§6，spec §2.6）。

---

## 10. MVP 設定檔（`projects.yaml`）

與 spec §9.3 相同；啟動時載入並 upsert **`projects` 表**：

```yaml
projects:
  - name: game-backend
    repo_path: /data/reviewer/repos/game-backend
    git_remote_url: git@gitlab.example.com:team/game-backend.git  # 選填
```

---

## 11. 與 spec 待決議題的對應

### 已決（schema 已對齊）

| spec # | 議題 | 本 schema 的處理 |
|--------|------|------------------|
| — | 部署形態 | `DATA_ROOT_DIR` + `repo_path`；spec §0、§9.0 |
| 1 | 排程器 | `schedule_config` + 後端 cron；`runs.trigger='schedule'` |
| 2 | 趨勢資料 | 讀檔 §0、§6；**無** trend 表 |

### 待決

| spec # | 議題 | 本 schema 預留 / 備註 |
|--------|------|----------------------|
| 3 | 重點版格式 | 傾向獨立 `summary.md`（§7） |
| 4 | 長期觀察重算 | `_cache/long_term.md` 檔案快取 |
| 5 | 漏跑補償 | `runs` 查詢 §9 |
| 6 | identity 衝突 | `person_identities.UNIQUE(kind, value)` |
| 7 | Repo 同步 | `projects.git_remote_url` |
| 8 | 多使用者認證 | 不在本 schema；spec §8 #8 |

---

## 附註

- 所有時間欄位用 TEXT 存 ISO8601（SQLite 慣例），伺服器 UTC 存、前端轉本地。
- 外鍵需啟用：連線時 `PRAGMA foreign_keys = ON;`。
- 報告檔與 DB 分離：刪報告列不自動刪 md 檔（保留存檔）；如需清理另做維護工作。
- 建議建立 `schema_version` 單列表，預留 migration。
- headless workflow 與後端**不共享 DB**；workflow 只寫檔，後端寫 manifest + 讀檔 + 寫 SQLite（spec §6.0）。
