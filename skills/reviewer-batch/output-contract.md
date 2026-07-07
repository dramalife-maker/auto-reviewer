# output-contract.md — summary.md 輸出契約

> 後端 `reviewer-server` 會解析此格式寫入 `reports` 與 `pending_items`。  
> **heading 名稱與 frontmatter 鍵名為固定契約**；變更需同步修改後端 `summary.rs`。

---

## 檔案位置

```
{report_root}/{person}/{run_date}/summary.md
```

- `{report_root}` ← manifest `report_root`
- `{person}` ← manifest `authors[].display_name`（canonical 顯示名；與目錄名一致）
- `{run_date}` ← manifest `run_date`（與 frontmatter `date` 一致）

同目錄必須另有 `report.md`（完整版，格式自由）。

---

## 完整範本

```markdown
---
person: Alice
project: game-backend
date: 2026-07-05
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

---

## Frontmatter（YAML）

| 鍵 | 必填 | 型別 | 說明 |
|----|------|------|------|
| `person` | 是 | string | 工程師 canonical 顯示名；**必須**等於 manifest `authors[].display_name` 與 `{person}` 目錄名 |
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

三個 **level-2 heading 名稱不可改**：

| Heading | 用途 | 後端行為 |
|---------|------|----------|
| `## 本週重點` | 2–5 條 bullet | API 渲染；不入庫 |
| `## 成長面向` | 1–3 條 bullet | API 渲染；不入庫 |
| `## 待確認` | 0–5 條 bullet | 每條 `- ` → 一筆 `pending_items` |

Bullet 規則：

- 每條以 `- ` 開頭（hyphen + space）。
- 空 section 仍保留 heading，下方可無 bullet。
- section 之間不要插入其他 level-2 heading。

---

## 後端解析對照

| 產出 | 寫入 |
|------|------|
| frontmatter | `reports.one_line`, `mr_count`, `commit_count`, 路徑 |
| `person` | 查既有 `people.display_name`；未知則跳過該 summary |
| `## 待確認` bullets | `pending_items`（`raised_date` = `date` 的 `YYYY-MM`） |
| 檔案路徑 | `reports.summary_md_path`, `reports.report_md_path` |

---

## 常見錯誤（避免）

- ❌ 用 `### 本週重點` 或 `# 本週重點` 代替 `##`
- ❌ 用 `*` 或 `1.` 代替 `- ` bullet
- ❌ `date` 與 manifest `run_date` 不一致
- ❌ `project` 與 manifest `project_name` 不一致
- ❌ 將 summary 寫到錯誤目錄層級（缺少 `{run_date}/`）
- ❌ 在 summary 內寫 HTML

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
```
