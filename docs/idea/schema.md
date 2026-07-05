# 1on1 Reviewer 雲端 Web 服務 — 資料 Schema 附錄

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
    └── <name>/                      # 通常 = projects.name
        └── <person>/                # 通常 = people.display_name
            ├── index.md             # 趨勢「長期觀察」
            ├── YYYY-MM.md           # 趨勢「成長軌跡」素材
            ├── _notes.md            # 趨勢「歷史待確認」
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
    kind        TEXT    NOT NULL,            -- 'email' | 'glab_user'
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
> - **管理者手動閉環**：在 web 介面標 `resolved`（可寫回 skill 維護的 `_notes.md`，實作時定同步策略）。
> - **趨勢 Tab「歷史待確認」**：**主資料源為檔案**（`_notes.md`、各月 `YYYY-MM.md` / `summary.md` 的待確認區段）；DB 列為操作輔助，非 trend 主存儲。

---

## 4. 執行紀錄表

### 4.1 `runs` — 每次批次 / 單專案執行

```sql
CREATE TABLE runs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger     TEXT    NOT NULL,            -- 'schedule' | 'manual_all' | 'manual_project' | 'mr_poll'
    status      TEXT    NOT NULL DEFAULT 'running', -- 'running' | 'success' | 'partial' | 'failed'
    started_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT,
    duration_sec INTEGER,
    project_total INTEGER,                   -- 本次涵蓋專案數
    project_skipped INTEGER NOT NULL DEFAULT 0, -- 逾時跳過數
    note        TEXT
);

CREATE INDEX idx_runs_started ON runs(started_at DESC);
```

> `status='partial'`：有專案逾時跳過（規格 §7.2）。`trigger='mr_poll'` 供軌道 2 輪詢紀錄（可選）。

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
  "run_date": "2026-07-05",
  "since": "2026-06-28",
  "output_contract": "output-contract.md"
}
```

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
| 長期觀察 | `index.md` + 歷史 `YYYY-MM.md` | 讀檔；可選 AI 跨檔綜合（重算頻率：待決 #4） |
| 成長軌跡 | 歷史 `YYYY-MM.md` | 解析 / AI 跨季綜合，產時間線 JSON 回前端 |
| 歷史待確認 | `_notes.md` + 月檔待確認區段 | 解析結構化列表；可與 `pending_items` 對照 |

**可選快取（不落 SQLite）**：AI 重算的「長期觀察」可寫入 `reports/<name>/<person>/_cache/long_term.md`（spec §8 #4）。

**headless workflow 責任**：週報 / MR review 執行時維護 `index.md`、`YYYY-MM.md`（對齊既有 reviewer 輸出），後端只讀不寫（閉環標記除外）。

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
```

解析規則：
- **frontmatter**（`---` 區塊）：抽 `person` / `project` / `date` / `one_line` / `mr_count` / `commit_count` → 寫入 `reports` 對應欄位。
- **`## 待確認`** 下每個 `-` 項 → 一筆 `pending_items`（`raised_date` = frontmatter `date` 的 YYYY-MM）；workflow 亦應寫入月檔 / `_notes.md` 供趨勢讀檔。
- **`## 本週重點` / `## 成長面向`**：API 直接回傳 md 或渲染結果，不需入庫（內容已在 summary.md）。
- heading 名稱為固定契約，workflow 與後端都依賴；變更需同步雙方。

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

-- 漏跑補償檢查（後端服務啟動時）
SELECT MAX(started_at) AS last_run FROM runs WHERE trigger = 'schedule';
-- 與 schedule_config 推算的「上次應執行時間」比較，若無對應紀錄則提示補跑
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
