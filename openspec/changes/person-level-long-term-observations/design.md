## Context

`docs/idea/spec.md` 定義報告閱讀器以「人」為中心，總覽 Tab 跨專案合併本週 `summary`，趨勢 Tab 提供「長期觀察 / 成長軌跡 / 歷史待確認」三區塊且**讀檔不入 SQLite**（§2.6）。現行 MVP 僅實作本週總覽；趨勢資料在 schema 中指向 `reports/{project}/{person}/index.md` 等檔案（`docs/idea/schema.md` §0、§6）。

使用者指出：長期觀察應描述**這個人整體**（跨專案成長、協作風格、職涯軌跡），而非單一 repo 的技術細節。現行每專案一份 `index.md` 導致敘事破碎。另有一批舊 1on1 筆記格式不齊，無法對齊 `summary.md` 契約，但仍應能作為人物脈絡種子資料遷入。

## Goals / Non-Goals

**Goals:**

- 建立人物層目錄 `reports/_people/{display_name}/` 作為跨專案長期觀察的**唯一主資料源**（趨勢 Tab）。
- 後端提供 `GET /api/people/:id/trends` 讀檔回傳三區塊內容。
- 前端在人員檢視新增「趨勢」區塊（次層或 Tab），與既有「本週」並存。
- `reviewer-batch` workflow 每週產報後維護人物層 `index.md`、`YYYY-MM.md`、`_notes.md`（跨專案綜合）。
- 文件化寬鬆遷移：自由格式 Markdown 可直接放入人物層 `index.md`。

**Non-Goals:**

- 自動轉換舊資料到 `summary.md` 格式。
- 自動合併既有專案層 `index.md` 到人物層（僅文件指引）。
- 趨勢數量圖表、跨人比較。
- 修改 `pending_items` 跨專案語意（本週仍由 SQLite；歷史讀 `_notes.md`）。
- 人員合併 API。

## Decisions

### 人物層目錄使用 `_people` 前綴

- **選擇**：`$DATA_ROOT_DIR/reports/_people/{display_name}/`，`display_name` 與 `people.display_name` 一致（與專案層 `{person}` 目錄命名規則相同）。
- **理由**：底線前綴避免與 `projects.yaml` 的 `name` 衝突（專案目錄與人物目錄同層）；掃描 `reports/` 時可排除 `_people`。
- **替代**：`people/` 無前綴 — 若未來有專案名為 `people` 會碰撞。

### 專案層 `index.md` 降級為可選補充；`YYYY-MM.md` 兩層皆維護

- **選擇**：workflow **必須**更新專案層 `{report_root}/{person}/YYYY-MM.md`（本 repo 月度成長）與人物層 `{person_report_root}/{display_name}/YYYY-MM.md`（跨專案綜合）。專案層 `index.md` 仍為可選技術脈絡。趨勢 API **只讀**人物層月檔。
- **理由**：週報 `## 成長面向` 是本週切片；要了解人員成長需月度累積。專案層月檔保留單 repo 顆粒度，人物層月檔供趨勢 Tab 與跨專案 1on1。
- **替代**：僅人物層月檔 — 失去「在每個專案分別長大了什麼」的記錄，不採用。

### 趨勢 API 讀檔、不寫 DB

- **選擇**：`person_trends` 模組純讀檔組 JSON；與 spec §2.6 一致。
- **理由**：長期敘事變動慢、以 Markdown 為準；避免 trend 表漂移。
- **替代**：SQLite 快照 — 與既有架構決策衝突。

### manifest 擴充 `person_report_root`

- **選擇**：`RunManifest` 新增 `person_report_root` 指向 `reports/_people/{display_name}/`（每位 author 各自路徑，或共用根 + workflow 依 `display_name` 組路徑）。
- **理由**：workflow 需知道人物層寫入位置；與 `report_root`（專案層）並列。
- **替代**：workflow 自行推算路徑 — 易與後端不一致。

### 寬鬆遷移僅文件 + 讀檔容錯

- **選擇**：`index.md` 無 frontmatter 亦可顯示為「長期觀察」純 Markdown；不要求 `mr_count`。
- **理由**：舊資料遷移門檻低；structured 週報仍走 `summary.md` 契約。
- **替代**：強制 schema 驗證 — 阻礙遷移。

## Implementation Contract

### 人物層目錄佈局

- **路徑**：`{DATA_ROOT_DIR}/reports/_people/{display_name}/`
- **檔案**：
  - `index.md` — 長期觀察（自由 Markdown；workflow 每週追加跨專案段落）
  - `{YYYY-MM}.md` — 月度成長軌跡素材
  - `_notes.md` — 歷史待確認（`- [YYYY-MM] 問題` 格式）
  - `_archive/`（選填）— 使用者手動放置舊筆記，API 可選讀入 `index.md` 或忽略
- **命名**：`display_name` 必須與 `people.display_name` 完全一致；後端以 `person_id` 查 `display_name` 再組路徑。
- **失敗**：目錄或檔案不存在 → API 回傳空字串或空陣列，HTTP 200（非 404）。
- **驗收**：建立 `reports/_people/Alice/index.md` 後，`GET /api/people/:id/trends` 回傳 `long_term_observation` 含其內容。

### Trends API

- **端點**：`GET /api/people/:id/trends`
- **回應形狀**：
  ```json
  {
    "person_id": 1,
    "display_name": "Alice Chen",
    "long_term_observation": "<markdown string or empty>",
    "growth_timeline": [{ "month": "2026-07", "excerpt": "..." }],
    "historical_pending": ["- [2026-06] 問題文字"]
  }
  ```
- **行為**：
  - `long_term_observation` ← 讀 `_people/{name}/index.md` 全文
  - `growth_timeline` ← 列舉 `_people/{name}/` 下符合 `^\d{4}-\d{2}\.md$` 的檔案，依檔名降序，每檔取首段或全文摘要（實作可簡化為全文）
  - `historical_pending` ← 讀 `_notes.md` 中以 `- [` 開頭的行
- **失敗**：未知 `person_id` → HTTP 404。
- **驗收**：integration test 寫入 fixture 檔案後 API 回傳預期欄位。

### 前端趨勢檢視

- **行為**：選人後可切換「本週 / 趨勢」；趨勢呼叫上述 API 渲染三區塊（純質性，無圖表）。
- **空狀態**：無檔案時顯示「尚無長期觀察資料」提示與遷移文件連結（README 錨點）。
- **驗收**：手動 smoke — 選有 `index.md` 的人員可見長期觀察文字。

### reviewer-batch workflow 變更

- **行為**：每位已歸戶 author 產完專案層 `{date}/summary.md` 後，**追加**更新專案層 `{report_root}/{person}/YYYY-MM.md`（本 repo 月度成長），以及人物層 `_people/{display_name}/index.md`、`YYYY-MM.md`、`_notes.md`（跨專案綜合，引用本週各專案重點，非複製整段 summary 或專案月檔全文）。
- **manifest**：含 `person_report_root` 或等價欄位讓 workflow 知道人物層根路徑。
- **驗收**：mock run 後專案層與人物層 `YYYY-MM.md` 存在且時間戳更新。

### 寬鬆遷移文件

- **行為**：`docs/idea/migration-person-observations.md` 說明：舊筆記可直接貼入 `_people/{name}/index.md`；不需 `summary.md`；觸發入庫僅針對符合契約的週報。
- **驗收**：文件審查 — 路徑範例與 `DATA_ROOT_DIR` 一致。

## Risks / Trade-offs

- **[Risk] display_name 改名會斷路徑** → 文件註明改名需手動搬移 `_people/` 目錄；未來 change 可做 rename hook。
- **[Risk] 專案層與人物層內容重複** → workflow 指引：人物層寫跨專案綜合，專案層寫該 repo 細節。
- **[Risk] 趨勢 Tab 與 MVP 範圍** → 本 change 實作最小趨勢檢視，不實作 spec 全部 v3 控制台功能。

## Migration Plan

1. 部署後既有專案層 `index.md` **保留**，趨勢頁改讀人物層（可能為空）。
2. 管理者依文件將跨專案舊筆記放入 `_people/{name}/index.md`。
3. 下次「全部執行」成功後 workflow 開始維護人物層檔案。
4. Rollback：關閉趨勢 API 路由與前端 Tab；人物層檔案保留不刪。

## Open Questions

- `growth_timeline` 是否需 AI 即時跨檔綜合，或先回傳各 `YYYY-MM.md` 原文？（本 change：**原文**，不重算。）
- 是否在 UI 暴露 `_archive/` 內容？（本 change：**否**，僅文件提及。）
