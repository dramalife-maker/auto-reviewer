# scan-mrs-headless — Headless MR Review Workflow

> **模式**：`manifest.mode = mr_poll`（每次子行程僅處理 **單一** `mr_iid`）  
> **執行環境**：非互動、headless。後端已將 `cwd` 設為 `manifest.repo_path`（resident worktree）。  
> **邊界**：只讀 git repo、manifest 與報告目錄；**不查 SQLite、不問使用者、不寫 GitLab**。

---

## 0. 啟動（必做）

1. 讀取使用者 prompt 中的 manifest **絕對路徑**，用 Read 工具開啟 `manifest.json`。
2. 解析並記住：
   - `project_name`
   - `repo_path`（應與目前 cwd 一致）
   - `mr_iid`（本次子行程唯一目標 MR）
   - `draft_dir`（MR review 草稿落檔目錄）
   - `pending_dir`（觀察片段根目錄，例 `reports/<project>/<person>/_pending/`）
   - `reviewer_username`（GitLab 視角，可選）
   - `since`（可選；分析窗口起日）
   - `eligible_mrs_path`（triage 輸出 JSON；若未指定，預設與 manifest 同目錄的 `eligible_mrs.json`）
   - `output_contract`（固定為 `output-contract.md`）
3. Read `eligible_mrs.json`，從 `eligible[]` 取出與 `manifest.mr_iid` 相符的項目，取得 `mr_title`、`source_branch`、`author_identity`、`review_round`。
4. 若 `mode` 不是 `mr_poll`，或 `mr_iid` 缺失，停止並在 stderr 說明。

**硬性限制**：

- **禁止**以 glab 列舉 open MR（MR 發現已由 `scripts/triage-mrs.py` 完成）。
- **禁止**任何寫入 GitLab 的 glab 副作用（發佈 note、merge 等）。
- **禁止**互動式確認（不得詢問使用者是否發佈、合併或寫入）。
- 允許對**已指定的** `mr_iid` 執行 `glab mr diff`、`glab mr view` 取得 review 素材。
- 允許在 `repo_path` 內 Read git 歷史與原始碼；**禁止** Write repo 原始碼。

---

## 1. 準備 review 素材

以 manifest 的 `mr_iid` 與 `eligible_mrs.json` metadata 為範圍（不做 MR 列舉）：

1. `glab mr view <mr_iid> -F json -c` — MR 標題、描述、討論脈絡。
2. `glab mr diff <mr_iid>` — 變更 diff。
3. 在 `repo_path` 內以 Read / git 工具檢視相關檔案與歷史脈絡。
4. 若 `source_branch` 有對應 MR worktree（後端已 provision），可在該 worktree 內檢視；否則以 resident worktree 為主。

聚焦 **事實查核、同類模組對照、六大面向審查**（架構、正確性、可維護性、測試、安全、效能），語氣建設性、非評判式。

---

## 2. 產出 MR review 草稿

寫入 `{draft_dir}/mr-{mr_iid}-round-{review_round}.md`（檔名可含 slug，但 frontmatter 契約為準）。

**必須**符合 appended `output-contract.md`：

- YAML frontmatter 含 `mr_iid`、`mr_title`、`review_round`、`author_identity`（皆必填）
- Body 為完整 review 內容（Markdown），供收件匣顯示與日後發佈

`author_identity` 使用 `eligible_mrs.json` 帶入值（通常為 MR author email 或 glab username）。

---

## 3. 產出工程師觀察片段

將本 MR 的**工程師行為觀察**（非完整 review 複製）寫入觀察片段：

```
{pending_dir}/mr-{mr_iid}-round-{review_round}.md
```

建議內容（自由 Markdown，無固定 heading 契約）：

- MR 編號與標題
- 1–3 條可帶入 1on1 的觀察（協作風格、技術判斷、review 互動等）
- 語氣建設性；避免純產量評論

片段由週報軌道消費；**僅在管理者發佈對應 MR review 後**才會折入週報（見 `spec.md §6.5`）。

---

## 4. 品質與風格

- **語言**：繁體中文（技術名詞、程式識別符可保留英文）。
- **分輪語意**：
  - `review_round: 1` — 首次 review，完整覆蓋變更。
  - `review_round: 2` — 追蹤作者回應後的新 commit／討論，聚焦差異與是否解決先前問題。
- **禁止**：要求使用者輸入、輸出「請確認是否寫入」、修改 SQLite、列舉其他 MR。

---

## 5. 結束

- 草稿與觀察片段寫入完成後正常 exit。
- 不要輸出 session 摘要給人類；產物以檔案為準。
- 不要自行寫入 `session_id` 至 frontmatter（由後端從 subprocess stdout 擷取）。

---

## 6. 快速檢查清單

- [ ] 已 Read manifest.json 與 `eligible_mrs.json`
- [ ] 僅處理 manifest 指定的單一 `mr_iid`
- [ ] 未以 glab 列舉 open MR
- [ ] 未執行任何寫入 GitLab 的 glab 指令
- [ ] 草稿 frontmatter 含 `mr_iid` / `mr_title` / `review_round` / `author_identity`
- [ ] 草稿寫入 `draft_dir`、觀察片段寫入 `pending_dir`
- [ ] 未互動詢問、未 Write repo 原始碼
