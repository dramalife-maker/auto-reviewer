# output-contract.md — MR review 草稿輸出契約

> 後端 `reviewer-server` 掃描 `draft_dir` 後解析此格式，upsert 進 `mr_reviews`（`status='draft'`）。  
> **frontmatter 鍵名為固定契約**；變更需同步修改後端 MR draft 解析器。

---

## 檔案位置

```
{draft_dir}/mr-{mr_iid}-round-{review_round}.md
```

- `{draft_dir}` ← manifest `draft_dir`
- 檔名建議含 `mr_iid` 與 `review_round`，但解析以 frontmatter 為準

---

## 完整範本

```markdown
---
mr_iid: 42
mr_title: "feat: add cache layer"
review_round: 1
author_identity: alice@co.com
---

# MR !42 — feat: add cache layer

## 總覽
（變更目的與影響範圍）

## 優點
- ...

## 問題與建議
- ...

## 測試與風險
- ...
```

---

## Frontmatter（YAML）

| 鍵 | 必填 | 型別 | 說明 |
|----|------|------|------|
| `mr_iid` | 是 | integer | GitLab MR internal id（!number） |
| `mr_title` | 是 | string | MR 標題 |
| `review_round` | 是 | integer | `1` 或 `2`（由 triage script 判定） |
| `author_identity` | 是 | string | MR author email 或 glab username；後端比對 `person_identities` 得 `person_id` |

規則：

- 以 `---` 開頭與結束。
- 缺 `mr_iid` 或 `review_round` 的檔案會被跳過並記 warning。
- **不要**在 frontmatter 寫入 `session_id`（由後端從 agent stdout 擷取）。

---

## Body（Markdown）

- 自由格式 Markdown，作為收件匣 `draft_body` 與日後 `glab mr note` 發佈內容來源。
- 建議含總覽、優點、問題與建議、測試與風險等段落，但無固定 heading 契約（與週報 `summary.md` 不同）。

---

## 觀察片段（另檔）

工程師觀察寫入 manifest `pending_dir`（非本契約檔案）：

```
{pending_dir}/mr-{mr_iid}-round-{review_round}.md
```

格式自由；供週報 `reviewer-batch` 消費。僅對應 `mr_reviews.status='published'` 時才折入週報。

---

## 後端解析對照

| 產出 | 寫入 |
|------|------|
| frontmatter `mr_iid` / `review_round` | `mr_reviews` upsert key `(project_id, mr_iid, review_round)` |
| `author_identity` | 查 `person_identities` → `person_id`（比對不到則 `NULL`） |
| 檔案路徑 | `mr_reviews.draft_md_path` |
| body | 收件匣顯示；發佈時可編輯後 POST 至 GitLab |
