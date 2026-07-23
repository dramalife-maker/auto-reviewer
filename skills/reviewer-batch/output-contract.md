# output-contract.md — summary.md 輸出契約

> 後端 `reviewer-server` 會解析此格式寫入 `reports` 與 `pending_items`。  
> **heading 名稱與 frontmatter 鍵名為固定契約**；變更需同步修改後端 `summary.rs`。

---

## 檔案位置

```
{report_root}/{person}/{run_date}/summary.md
```

- `{report_root}` ← manifest `report_root`
- `{person}` ← manifest `authors[].folder_name`（不可變路徑鍵；即目錄名。**非** `display_name`）
- `{run_date}` ← manifest `run_date`（與 frontmatter `date` 一致）

同目錄必須另有 `report.md`（完整版，格式自由）。

---

## 完整範本（填滿；含觀察折入）

```markdown
---
person: Gary Tsai
project: game-backend
date: 2026-07-05
one_line: 本週主導 TS wallet 與 kot endpoints；可靠性設計強，邊界收尾與清單外自審仍需盯，有 1 項待確認。
mr_count: 2
commit_count: 18
---

## 本週重點
- 主導 FunTa TS wallet（!68）：`packet_id` 冪等與掉包恢復設計完整，測試覆蓋到位
- 完成 kot DataTally Check/Export 與線上人數統計；敏感資料邊界（排除 token）意識清楚
- Review 被點名的項目執行力強；同批夾帶的 `GetPortal` 曾缺 pool 設定（第二輪已列修法）

## 成長面向
- 「清單內照做、清單外自審偏弱」模式再次出現（!68 夾帶功能、!73 float64 金額需 reviewer 點出）— 宜在 1on1 談合併前自檢清單
- 主導設計的 greenfield 模組完成度高（可靠性建模），與順手支線的品質落差明顯

## 待確認
- 合併前自檢（金額型別、錯誤碼一致性、清單外變更測試）要如何內化成習慣而非靠 reviewer 攔？

## 已釐清
```

---

## Frontmatter（YAML）

| 鍵 | 必填 | 型別 | 說明 |
|----|------|------|------|
| `person` | 是 | string | 工程師不可變路徑鍵；**必須**等於 manifest `authors[].folder_name` 與 `{person}` 目錄名。**不可**使用 `display_name`（顯示名可能已改，僅供正文稱呼） |
| `project` | 是 | string | 必須等於 manifest `project_name` |
| `date` | 是 | string | `YYYY-MM-DD`；必須等於 manifest `run_date` |
| `one_line` | 是 | string | 一兩句話摘要 |
| `mr_count` | 否 | integer | 本窗口 MR 參與數；未知可省略 |
| `commit_count` | 否 | integer | 本窗口 commit 數（非 merge） |

規則：

- 以 `---` 開頭與結束。
- 使用 YAML 1.1 相容語法；字串含冒號時加引號。

---

## Body sections（Markdown）

四個 **level-2 heading 名稱不可改**（順序固定）：

| Heading | 用途 | 後端行為 |
|---------|------|----------|
| `## 本週重點` | 2–5 條 bullet | API 渲染；不入庫 |
| `## 成長面向` | 1–3 條 bullet | API 渲染；不入庫 |
| `## 待確認` | 0–5 條 bullet | 每條 `- ` → 一筆 `pending_items`（ingest 對 open 同文去重） |
| `## 已釐清` | 0–N 條 bullet | 每條精確匹配 open `(person, project, question)` → resolve（含 `_notes.md`）；無匹配則忽略 |

Bullet 規則：

- 每條以 `- ` 開頭（hyphen + space）。
- 空 section 仍保留 heading，下方可無 bullet。
- section 之間不要插入其他 level-2 heading。
- **延續既有 open**：寫入 `## 待確認` 時文字必須等於 `manifest.open_pending[].question`。
- **已釐清**：寫入 `## 已釐清` 時文字必須等於對應 open `question`，且不得同時出現在 `## 待確認`。僅省略兩區 ≠ resolve。詳見 `WORKFLOW.md`「待確認延續規則」。
- **專案 ADR**：寫新的技術選擇類 `## 待確認` 前必須遵守 `WORKFLOW.md`「專案 ADR（notes_dir）」；已知決策不得再寫成新問句。

### 寫作規範（觀察折入）

- 有 `published_pending_snippets` 時：**成長面向**優先吸收場次紀錄的「觀察到的思維模式」（改寫 1–3 條，附 MR 錨點即可）。
- **本週重點**寫交付與協作結果；不要貼觀察的審查意見表或草稿 ❌ 全文。
- 建設性、非評判式；避免純產量排名。

---

## 後端解析對照

| 產出 | 寫入 |
|------|------|
| frontmatter | `reports.one_line`, `mr_count`, `commit_count`, 路徑 |
| `person` | 查既有 `people.folder_name`；未知則跳過該 summary |
| `## 待確認` bullets | `pending_items`（`raised_date` = `date` 的 `YYYY-MM`） |
| `## 已釐清` bullets | 匹配 open 列 → `status=resolved` + 同步 `_notes.md` |
| 檔案路徑 | `reports.summary_md_path`, `reports.report_md_path` |

---

## 常見錯誤（避免）

- ❌ 用 `### 本週重點` 或 `# 本週重點` 代替 `##`
- ❌ 用 `*` 或 `1.` 代替 `- ` bullet
- ❌ `date` 與 manifest `run_date` 不一致
- ❌ `project` 與 manifest `project_name` 不一致
- ❌ 將 summary 寫到錯誤目錄層級（缺少 `{run_date}/`）
- ❌ 在 summary 內寫 HTML
- ❌ 整段貼上 `_pending` 場次紀錄或 MR 草稿表格
- ❌ 每週無重複模式仍改寫人物層 `index.md` 長文

---

## 最小合法範例（無待確認）

```markdown
---
person: Bob
project: web-portal
date: 2026-07-05
one_line: 本週僅小幅維護，無重大風險。
commit_count: 3
---

## 本週重點
- 修正登入頁 race condition

## 成長面向
- 測試覆蓋率穩定

## 待確認

## 已釐清
```
