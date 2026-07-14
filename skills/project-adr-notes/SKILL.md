# project-adr-notes — 專案 ADR 寫入契約（agent chat）

> **用途**：MR 收件匣 agent-turn（`--resume`）附加此檔。管理者**顯式**要求記錄決策時，寫入 `notes_dir` 下的專案 ADR。  
> **非用途**：週報／MR headless 掃描**只讀** ADR；本 skill 的寫入規則**不**適用於那些 headless 行程。

回合上下文會帶 `notes_dir=<絕對路徑>`（例 `…/reports/game-backend/.notes`）。所有 ADR 讀寫都在此目錄下。

---

## 1. 顯式寫入觸發（硬性）

**只有**管理者訊息含下列白名單（或其明顯變體，如「把這個記成 ADR」）才可寫入 `.notes/`：

| 允許觸發 | 例 |
|----------|-----|
| 記成 ADR | 「把這個記成 ADR」「請記成 ADR」 |
| 寫入決策 | 「寫入決策」「把決策寫進去」 |
| record as ADR | `record as ADR` / `please record as ADR` |

### 正例

管理者先說明「session 用 Redis」，再說「把這個記成 ADR」→ **寫** ADR + 更新 index，回覆兩個路徑。

### 反例

管理者只說「我們選 Redis 是因為要水平擴」→ **不寫** ADR、不改 index（即使對話已收斂）。

無觸發時：**禁止**建立 `adr-*.md`、**禁止**改 `index.md`。

---

## 2. 目錄與檔名

```
{notes_dir}/
  index.md                 # 僅索引
  adr-YYYYMMDD-slug.md     # 單則正文
```

- `slug`：小寫 kebab，檔名安全（英文或拼音）。
- `.notes` 是專案報告根下的**保留目錄**；**不是**工程師 `display_name` 資料夾。
- Person 待確認歷史仍只寫 `reports/_people/{display_name}/_notes.md`——**不要**把 ADR 寫進 `_people`。

寫入順序（防半套）：

1. 若缺目錄 → `create_dir_all` `{notes_dir}`
2. 寫入新的 `adr-YYYYMMDD-slug.md`
3. Read 既有 `index.md`（若有）→ **只追加一列**（或建新檔用下方範本）
4. 回覆宣告：ADR 路徑 + index 已更新

若步驟 3 失敗：回覆警告，並指出步驟 2 已寫出的孤兒檔需手動修。

---

## 3. ADR 正文格式（禁止 YAML／`<meta>`）

```markdown
# Session 用 Redis 而非記憶體

<tl;dr>
- **何時要想起這則：** 改 session／登入態儲存，或要新開另一套 cache 當 session 時。
- **決策：** Session 存 Redis，不用 process 記憶體。
- **不要做／不要再問：** 再用 in-memory session；週報／MR 再問「為何選 Redis」。
- **要做：** 沿用既有 Redis session 客戶端與 key 慣例。
- **意圖：** 多實例可擴、重啟不丟登入態。
- **自問（可選）：** 這次是否又發明第二套 session store？
</tl;dr>

## 為何這樣選（意圖）

管理者在討論中確認的取捨（一句到一小段）。

## Context

（題目怎麼來）

## Decision

（選定方案＋理由，可比 tl;dr 稍詳）

## Consequences

（約束、例外何時可重開討論）
```

`<tl;dr>` **必填鍵**：何時要想起這則、決策、不要做／不要再問、要做、意圖。  
`<tl;dr>` **禁止**出現：`date`、`status`、`source`、`mr_iid`（以及 YAML frontmatter、`<meta>`）。  
日期若要掃：看檔名 `adr-YYYYMMDD-…`。

---

## 4. index.md（禁止整檔覆寫）

### 初始範本（**僅當檔案不存在**時建立）

```markdown
# Project ADRs

快速查詢本專案技術決策。每次新增 ADR 後，同步更新此索引（只追加列）。

| 日期 | 檔案 | 摘要 |
|------|------|------|
```

### 更新規則

- **變更前必須** Read 現有 `index.md` 全文。
- **只**在表格**新增一列**；**不得**用範本或空白表整檔覆寫。
- 日期欄取自檔名 `adr-YYYYMMDD-…` 的 `YYYY-MM-DD` 形式（例 `20260714` → `2026-07-14`）。
- 摘要：一句話（通常用 H1 標題）。

例：

```markdown
| 2026-07-14 | adr-20260714-redis-session.md | Session 用 Redis 而非記憶體 |
```

---

## 5. 回覆

成功：簡短確認 + ADR 相對／絕對路徑 +「index 已追加一列」。  
失敗：說明哪一步失敗；不要靜默吞錯。
