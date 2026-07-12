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
   - `prior_published_reviews`（可選陣列；後端注入的**已發佈**前輪 AI review，依 `review_round` 由舊到新）
   - 後端會 append 兩份規格檔（勿與彼此搞混）：
     - `output-contract.md` — **MR 草稿**（收件匣／日後 GitLab note）
     - `observation-guidelines.md` — **觀察片段**（管理者／週報／1on1）
3. Read `eligible_mrs.json`，從 `eligible[]` 取出與 `manifest.mr_iid` 相符的項目，取得 `mr_title`、`source_branch`、`author_identity`、`review_round`。
4. 若 `mode` 不是 `mr_poll`，或 `mr_iid` 缺失，停止並在 stderr 說明。

**硬性限制**：

- **禁止**以 glab 列舉 open MR（MR 發現已由 `scripts/triage-mrs.py` 完成）。
- **禁止**任何寫入 GitLab 的 glab 副作用（發佈 note、merge 等）。
- **禁止**互動式確認（不得詢問使用者是否發佈、合併或寫入）。
- 允許對**已指定的** `mr_iid` 執行 `glab mr diff`、`glab mr view` 取得 review 素材。
- 允許在 `repo_path` 內 Read git 歷史與原始碼；**禁止** Write repo 原始碼。
- 允許寫入：`draft_dir`（草稿）、觀察片段路徑（見 §3）、以及**僅在重複模式時**追加人物層長期檔（見 `observation-guidelines.md`）。
- 允許 Read：同人既有 `_pending/`／`_archived/`、人物層 `reports/_people/{display_name}/`（若存在）。

---

## 1. 準備 review 素材

以 manifest 的 `mr_iid` 與 `eligible_mrs.json` metadata 為範圍（不做 MR 列舉）：

1. `glab mr view <mr_iid> -F json -c` — MR 標題、描述、**完整討論脈絡**（含人工與先前 AI note）。**必做**。
2. `glab mr diff <mr_iid>` — 變更 diff。
3. 在 `repo_path`（或已 provision 的 MR worktree）內 **Read 完整相關檔案**，勿只看 diff hunk。
4. 若 `source_branch` 有對應 MR worktree（後端已 provision），優先在該 worktree 檢視；否則以 resident worktree 為主。

### 1.1 多輪（`review_round >= 2`）— 必做複習

當 `eligible` 的 `review_round` ≥ 2 時，**必須先建立「先前建議 → 目前狀態」對照**，再寫本輪草稿：

1. **讀取 `manifest.prior_published_reviews`**（若非空）：每一筆含 `review_round`、`published_at`（可選）、`body`（該輪實際發佈到 GitLab 的 AI review 全文）。由舊到新閱讀。
2. **對照 `glab mr view -c` 討論**：作者回覆、後續 commit 說明、人工 review 意見；確認哪些建議已回應、哪些仍開放。
3. 若 `prior_published_reviews` 為空（例如前輪從未透過本系統發佈），仍須以 GitLab notes 中含 `By: AI Agent` 的內容作為前輪依據，不可略過複習。

### 1.2 同類模組對照與整合文件（變更為新模組／整合層時必做）

1. 在 repo 內找 **1–2 個**已存在、同類別模組（同資料夾 sibling，或目錄結構相同的其他子系統）。
2. 對照分檔慣例：錯誤碼、router、路徑常數、設定檔、命名前綴。
3. 草稿中明確列出「與既有同類模組不一致」的項目（進 ❌ 或 ❓，見分輪策略）。
4. **整合文件**：若存在 `AGENTS.md`、`.claude/docs`、或與本次變更主題相關的整合／規格文件，先 Read 並對照需求。

### 1.3 審查視角與六大面向

以 **資深後端／技術主管 reviewer** 視角審查；依專案技術棧套用對應慣例（優先 `CLAUDE.md`／`AGENTS.md`／`.claude/docs`；若 repo 另有 style guide 文件則一併遵循）。

六大面向（草稿須覆蓋，不必各開一節）：

1. **Coding style** — 命名、格式、錯誤處理、import 風格
2. **Architecture** — 分層、依賴方向、模組邊界
3. **文件** — 註解、API 契約、migration 說明
4. **可簡化的邏輯** — 重複邏輯、過深分支、過度設計
5. **慣例對齊（Convention parity）** — 是否沿用同類模組結構；有無「重新發明一套」、死碼（定義未用的常數／error code）
6. **具體修改建議** — 每個 ❌ 含可操作修法（路徑、符號、範例）

語氣建設性、非評判式。

---

## 2. 產出 MR review 草稿

寫入 `{draft_dir}/mr-{mr_iid}-round-{review_round}.md`（檔名可含 slug，但 frontmatter 契約為準）。

**必須**符合 appended `output-contract.md`：

- YAML frontmatter 含 `mr_iid`、`mr_title`、`review_round`、`author_identity`（皆必填）
- Body 依契約段落：審查摘要 →（round 2+ 上一輪表）→ 做得好 → 需要修正表 → 建議追問 → 整體評估
- **整體評估只寫技術／合併判斷**；思維模式只寫觀察片段
- **不要**在草稿末加 `By: AI Agent`（發佈時後端附加）

`author_identity` 使用 `eligible_mrs.json` 帶入值（通常為 MR author email 或 glab username）。

### 2.1 分輪輸出策略

| 輪次 | 觸發 | 草稿輸出重點 |
|------|------|----------------|
| 第一輪 | 首次 review；integration 類；語意／文件不明 | **❓ 建議追問為主**；✅ 已對照部分；**少給** ❌ 修法 |
| 第二輪+ | 上次 AI review 後有新 commit 或作者回覆 | 先輸出「上一輪疑慮處理狀態」，再給本輪 ❌／❓ |

### 2.2 多輪（`review_round >= 2`）

複習素材見 §1.1。Body **必須**含 `### 📋 上一輪疑慮處理狀態` 表；**沿用上一輪編號**（`F1 → 已解`），不要重編。

本輪「需要修正」與「整體評估」只覆蓋：新 commit 風險、前輪未關閉項、討論新議題——避免重貼 round 1 全文。

### 2.3 寫入草稿前事實查核（必做）

任一準備寫入 ❌ 的 **`[高]`** 項，落檔前必須用程式路徑驗證；**不確定 → 改放 ❓，不寫進 ❌**。

Checklist：

- 錯誤實際回傳型別？`errors.Is`／對應語言等價檢查是否成立？
- 描述的程式路徑是否真的會被執行到？
- 是「設計取捨」還是「確定 bug」？分不清就降為 ❓
- 「像 AI 生成後未自審」這類判斷**只寫觀察片段**（可推測），**不要**在草稿 ❌ 當事實指控

---

## 3. 產出工程師觀察片段

寫入：

```
{pending_dir}/mr-{mr_iid}-round-{review_round}.md
```

**必須**遵循 appended `observation-guidelines.md`（與 §2 草稿分開的寫作規範）：

- 一場審查一篇**場次紀錄**；首選段落見該檔範本（背景／提案重點／審查意見／思維模式…；heading 可同義）
- **思維模式**須同時涵蓋協作回應與技術判斷，並綁本場證據
- **長期思維**（人物層 `index.md`）：僅當與過去 review 構成**重複模式**才追加；單次現象只寫本場、不要每次都改長期檔
- **禁止**把草稿 note 全文貼進觀察；**禁止**把觀察裡的對人敘事寫進草稿

片段由週報軌道消費；**僅在管理者發佈對應 MR review 後**才會折入週報。

---

## 4. 品質與風格

- **語言**：繁體中文（技術名詞、程式識別符可保留英文）。
- **分輪語意**：見 §2.1；round 1 偏 ❓、少 ❌；round 2+ 先追蹤再給本輪焦點。
- **兩產物分界**：草稿 = 對作者的 code review；觀察 = 對管理者的場次／思維紀錄。
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
- [ ] 已執行 `glab mr view … -c` 取得討論脈絡
- [ ] 已 Read 完整相關檔案；新模組／整合層已做同類模組對照（§1.2）
- [ ] 若 `review_round >= 2`：已讀 `prior_published_reviews`（或 GitLab AI notes）並寫入「上一輪疑慮處理狀態」表（沿用編號）
- [ ] 未以 glab 列舉 open MR
- [ ] 未執行任何寫入 GitLab 的 glab 指令
- [ ] 草稿 frontmatter 含 `mr_iid` / `mr_title` / `review_round` / `author_identity`
- [ ] 草稿 body 含契約段落；`[高]` 已事實查核；無思維模式、無自行加 `By: AI Agent`
- [ ] 草稿寫入 `draft_dir`（遵循 `output-contract.md`）
- [ ] 觀察片段寫入 `pending_dir`（遵循 `observation-guidelines.md`；含思維模式；非草稿複本）
- [ ] 未互動詢問、未 Write repo 原始碼
