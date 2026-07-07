## Context

`docs/idea/schema.md` 已定義 `people`、`person_identities`、`unmatched_authors` 與歸戶流程，但現行實作仍以 `summary.md` frontmatter 的 `person` 顯示名稱 upsert `people`（`backend/src/summary.rs`），reviewer-batch workflow 亦以 git `%an` 決定報告目錄（`skills/reviewer-batch/WORKFLOW.md` §1）。因此同一真人在不同 email、不同 repo、不同 Git host 會被拆成多位人員。

本變更將歸戶責任移回後端：以 `(kind, value)` 決定性比對，LLM 只負責敘事產出。

## 使用者情境（來自需求討論）

### 情境 A：同 repo、多個 git 帳號

Alice 在公司 repo 有時用 `alice@company.com`、有時用 `alice@gmail.com` commit。兩個 email 都應綁到 `people.display_name = "Alice Chen"`，週報與 1on1 內容合併在同一人下。

### 情境 B：跨 Git host、不同使用者名

Alice 在 GitLab 專案用 `alice_w`，在 GitHub 專案用 `alice-dev`，但 commit email 可能分別是 `alice@company.com` 與 `alice@gmail.com`。第一階段以 `git_email` 歸戶；若兩 email 都綁到同一人，兩邊週報仍匯入同一 `person_id`。`gitlab_user` / `github_user` 綁定留作手動補充或後續變更。

### 情境 C：管理者如何「新增人員」

人員不是憑空在 UI 建立後就有週報。正確流程：

1. **預先綁定（建議）**：`POST /api/people` 建立人員 → `POST /api/people/:id/identities` 綁定已知 email → 跑 review。
2. **事後指認**：先跑 review → 未歸戶 email 進佇列 → UI 綁定到新人員或既有人員 → 重跑 review。

未綁定前，該 email 的 commit **不產週報**。

## Goals / Non-Goals

**Goals:**

- 執行 review 前，後端在 worktree 內列舉本週 git authors，以 `git_email` identity 解析 canonical `person_id`。
- 未命中 identity 的 author 寫入 `unmatched_authors`，並在 manifest 標記為未歸戶。
- 提供 API 與最小 UI：列出未歸戶、建立人員、將 identity 綁定到既有人員。
- 調整 manifest 與 workflow：依 canonical `display_name` 分組產出報告，不再以 `%an` 自行歸戶。
- 調整 summary ingestion：以 resolved `person_id` 寫入 `reports`；`person` frontmatter 必須與 canonical `display_name` 一致。

**Non-Goals:**

- `gitlab_user` / `github_user` 自動解析（僅預留 `kind` 欄位與 API 形狀；不在本變更實作 MR/PR author 抓取）。
- 人員合併（merge two `people` rows + 搬移 reports）— 留後續變更。
- 自動啟發式歸戶（同網域 email、模糊名稱比對）。
- Bot / noreply email 智慧過濾（僅做小寫正規化）。

## Decisions

### 歸戶在後端、不在 LLM

- **選擇**：worker 在 spawn subprocess 前，於 target worktree 執行 `git log --format='%ae|%an'` 列舉 authors，後端查 `person_identities`。
- **理由**：歸戶是資料正確性問題，必須可重現、可測試；LLM 不應猜測帳號歸屬。
- **替代**：維持 workflow 以 `%an` 分組 — 無法處理跨 email / 跨平台。

### 第一階段僅實作 `git_email` kind

- **選擇**：`kind='git_email'`，`value` 為小寫 trim 後的 author email。
- **理由**：commit author email 在所有 git host 上最穩定；跨平台 username 需額外 API，超出 MVP。
- **替代**：同時實作 `gitlab_user` — 需 glab 整合，延後。

### 未歸戶 author 不產報告、不建立 person

- **選擇**：manifest `authors` 中 `person_id: null` 的 author，workflow **略過**不寫 summary；同時 upsert `unmatched_authors` 累計 `commit_count`。
- **理由**：避免產出無法歸戶的報告與幽靈人員；強制管理者先綁定。
- **替代**：建立「未歸戶」暫時 person — 會污染人員列表。

### manifest 擴充 `authors` 陣列

- **選擇**：`manifest.json` 新增 `authors: [{ email, git_name, person_id, display_name }]`；僅已歸戶者列入。
- **理由**：workflow 讀 manifest 即可知道要為誰產報告與目錄名稱，無需再跑 git 歸戶邏輯。
- **替代**：workflow 繼續自己跑 git log — 與後端決策重複且易分歧。

### `UNIQUE(kind, value)` 衝突處理

- **選擇**：綁定時若 `(kind, value)` 已存在且 `person_id` 不同，API 回 HTTP 409。
- **理由**：符合 schema §2.2 待決 #6 嚴格模式；避免一帳號兩人。
- **替代**：允許覆寫 — 風險高，不採用。

## Implementation Contract

### Author 解析（run 前）

- **行為**：對每個將執行的 project，在 supply 後的 worktree 內，列舉 `since`～`run_date` 視窗內有非 merge commit 的 unique `(email, name)` 對。email 正規化為小寫 trim。查 `person_identities WHERE kind='git_email' AND value=?`。
- **命中**：取得 `person_id` 與 `people.display_name` 作為 manifest `display_name`。
- **未命中**：`person_id=null`；upsert `unmatched_authors`（累計 `commit_count`、`last_seen`）；不建立 `people` 列。
- **失敗**：worktree 不可用 → 沿用既有 worker skip 行為，不影響其他 project。
- **驗收**：integration test 綁定 `alice@co.com` 後，同 email 不同 `%an` 的 commits 解析到同一 `person_id`。

### manifest.json 新欄位

- **形狀**：
  ```json
  {
    "mode": "weekly_batch",
    "project_name": "...",
    "authors": [
      { "email": "alice@co.com", "git_name": "Alice", "person_id": 1, "display_name": "Alice Chen" }
    ]
  }
  ```
- **規則**：`authors` 僅含已歸戶者；未歸戶者不列入。`display_name` 為 canonical 名稱，用於 `{report_root}/{display_name}/` 目錄。
- **驗收**：fixture worktree + 已綁 identity → manifest 含正確 `authors`；未綁定 email 不出現在 `authors`。

### Identity 管理 API

| 方法 | 路徑 | 行為 |
|------|------|------|
| GET | `/api/unmatched-authors` | 回傳 `unmatched_authors` 列表 |
| POST | `/api/people` | body `{ display_name }` 建立 person |
| POST | `/api/people/:id/identities` | body `{ kind, value, label? }` 綁定 identity；成功後從 `unmatched_authors` 移除同 `(kind,value)` |
| GET | `/api/people/:id/identities` | 列出該人的 identities |

- **失敗**：重複 `(kind,value)` → 409；person 不存在 → 404。
- **驗收**：綁定後 `GET /api/unmatched-authors` 不再含該筆；下次 run manifest 出現該 author。

### Workflow 契約變更

- **行為**：§1 改為讀 manifest `authors` 陣列，不再自行 `git log` 決定 person 列表。每位 `authors[].display_name` 產一份報告。`summary.md` frontmatter `person` 必須等於 `display_name`。
- **驗收**：mock manifest + workflow 產出目錄名與 frontmatter 一致。

### Summary ingestion 變更

- **行為**：解析 summary 時，以 `people.display_name` 查 `person_id`（frontmatter `person` 必須命中既有 person）；不再 `INSERT` 新 person。若 person 不存在 → 記錄 warning、跳過該 summary。
- **驗收**：兩個 email 綁到同一人後，兩份 summary（不同目錄但同 `person`）寫入同一 `person_id`。

### 前端

- **行為**：header 或側欄顯示未歸戶數量（`GET /api/unmatched-authors` length）。提供「未歸戶」面板：每筆可「建立新人員並綁定」或「綁定到既有人員」。
- **驗收**：手動測試綁定後未歸戶數下降；下次 run 後該人出現在左欄。

### Scope boundaries

- **In scope**：`git_email` 歸戶、manifest authors、unmatched 佇列、綁定 API、基本 UI、ingestion 改用 person_id。
- **Out of scope**：gitlab/github username kind、person merge、自動歸戶啟發式、重新處理歷史未歸戶報告。

## Risks / Trade-offs

- [未歸戶 author 被略過] → UI 明確提示未歸戶數；文件說明需先綁定再跑 review。
- [既有幽靈 people 列] → 不自動清理；後續可提供 merge 工具。
- [email 大小寫不一致] → 一律小寫正規化後比對。
- [manifest 與 workflow 版本不同步] → output-contract 與 WORKFLOW 同步更新；integration test 驗 manifest 形狀。

## Migration Plan

1. 部署後既有 `people` 列保留；管理者需為每位工程師手動建立 identity 綁定（或透過未歸戶佇列一次性綁定）。
2. 首次 run 後檢查 `unmatched_authors`，完成綁定後重跑。
3. 無 DB migration 需求（表已存在）；僅行為變更。

## Resolved Decisions

- `GET /api/people` **SHALL** 回傳 `identity_count`（已綁 identity 數）。
- `unmatched_authors.project_id` **SHALL** 必填，API 回傳 `project_name` 供按專案篩選。


