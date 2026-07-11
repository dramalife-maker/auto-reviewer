# reviewer-batch — Headless 週報 Workflow

> **模式**：`manifest.mode = weekly_batch`  
> **執行環境**：非互動、headless。後端已將 `cwd` 設為 `manifest.repo_path`。  
> **邊界**：只讀 git repo 與 manifest 指向的報告目錄；**不查 SQLite、不問使用者、不寫 GitLab**。

---

## 0. 啟動（必做）

1. 讀取使用者 prompt 中的 manifest **絕對路徑**，用 Read 工具開啟 `manifest.json`。
2. 解析並記住：
   - `project_name`
   - `repo_path`（應與目前 cwd 一致）
   - `report_root`（本專案報告根目錄，例 `$DATA_ROOT_DIR/reports/game-backend`）
   - `person_report_root`（人物層報告根目錄，例 `$DATA_ROOT_DIR/reports/_people`；每位 author 的跨專案長期檔寫入 `{person_report_root}/{display_name}/`）
   - `run_date`（本次報告日期，`YYYY-MM-DD`）
   - `since`（分析窗口起日，`YYYY-MM-DD`，含當日）
   - `authors`（已歸戶工程師陣列：`email`, `git_name`, `person_id`, `display_name`）
   - `open_pending`（本專案目前 `status='open'` 的待確認；元素含 `id`, `person_id`, `display_name`, `question`。寫 `## 待確認` 時必須遵守下方「待確認延續規則」）
   - `published_pending_snippets`（可選陣列；相對於 `report_root` 的路徑，例如 `Alice Chen/_pending/mr-42-round-1.md`。後端依 `mr_reviews.status='published'` 預先篩選；未列於此陣列的 `_pending/` 片段**不得**折入週報）
   - `output_contract`（固定為 `output-contract.md`，格式見同目錄 appended 規格）
3. 若 `mode` 不是 `weekly_batch`，停止並在 stderr 說明（仍 exit 0 除非無法讀 manifest）。

**寫入限制（硬性）**：

- 允許寫入：`{report_root}/**` 底下所有檔案，以及 `{person_report_root}/{display_name}/**`（跨專案長期檔）。
- 允許讀取：`repo_path` 下 git 歷史、`report_root` 與 `{person_report_root}/{display_name}/` 既有檔案（含 `_pending/`）。
- **禁止**寫入 manifest 路徑以外的新根目錄、禁止修改 `repos/` 原始碼（除 Read 外不 Write repo 內容）。

---

## 1. 盤點本週活躍工程師

**不要**自行執行 `git log` 決定人員列表。改為讀 manifest 的 `authors` 陣列：

```json
"authors": [
  { "email": "alice@co.com", "git_name": "Alice", "person_id": 1, "display_name": "Alice Chen" }
]
```

規則：

- 僅為 `authors` 中每位已歸戶工程師產出報告（後端已過濾未歸戶 email）。
- 目錄名與 summary frontmatter 的 `person` 欄位 **必須** 使用 `authors[].display_name`（canonical 名稱），**不可** 使用 `%an` 或 email。
- 若 `authors` 為空陣列，寫完 manifest 後正常結束（不視為錯誤）。
- 統計每位作者的 `commit_count`：在 `repo_path` 對該 author 的 email 執行 `git log`（`since`～`run_date`，`--no-merges`）。`mr_count` 若無法從 git 可靠取得，填 `0` 或省略（後端接受 null）。

---

## 2. 每位工程師：收集素材

對每位 active author（以下稱 `{person}`）：

### 2.1 Git 活動

- `git log` / `git show` / `git diff` 理解本週變更主題、技術深度、review 互動（若 log 可見）。
- 聚焦 **行為與成長**，非產量排名；避免「commit 多＝表現好」這類結論。

### 2.2 MR 觀察片段（可選）

軌道 2 會將觀察片段寫入 `{report_root}/{person}/_pending/`。**僅** manifest 的 `published_pending_snippets` 所列路徑可折入週報（對應 `mr_reviews.status='published'`）；`draft` / `ignored` 的片段必須留在 `_pending/` 不動。

對每位 `{person}`：

1. 從 `published_pending_snippets` 篩出以 `{person}/_pending/` 開頭的 `.md` 路徑（相對於 `report_root`）。
2. Read 這些片段，合併進本週分析（可併入 `report.md` 與 `summary.md` 的「本週重點」或「協作與 review」）。
3. 寫完本次報告後，將**已消費**的片段搬移至 `{report_root}/{person}/_pending/_archived/`（若目錄不存在則建立），不要刪除。
4. **不要** Read 或搬移未列於 `published_pending_snippets` 的 `_pending/` 檔案。

### 2.3 歷史脈絡（可選）

Read 若已存在：

- `{person_report_root}/{display_name}/index.md` — **跨專案**長期觀察（趨勢 Tab 主資料源）
- `{person_report_root}/{display_name}/YYYY-MM.md`（`run_date` 所在月份）— 跨專案月度成長
- `{person_report_root}/{display_name}/_notes.md` — 歷史待確認
- `{report_root}/{person}/index.md` —（可選）本專案技術脈絡補充
- `{report_root}/{person}/YYYY-MM.md`（`run_date` 所在月份）— **本專案**月度成長素材

用於延續敘事、避免與過去待確認矛盾；**不要**複製整段舊文到 summary。

---

## 3. 產出單次報告（每位工程師）

目錄：

```
{report_root}/{person}/{run_date}/
├── report.md      # 完整版，供深讀
└── summary.md     # 精簡版，必須符合 output-contract.md
```

### 3.1 `report.md`（完整版）

無固定 schema，建議結構：

```markdown
# {person} — {project_name} — {run_date}

## 本週概覽
（段落敘述）

## 技術與交付
（依 commit / MR 整理，可含具體模組、決策、trade-off）

## 協作與 review
（若可觀察）

## 風險與待確認
（詳細版；summary 會精簡）

## 參考
- commit: <hash> — <subject>
```

### 3.2 `summary.md`（精簡版 — 硬性契約）

**必須**完全符合 appended `output-contract.md`：

- YAML frontmatter 含 `person`, `project`, `date`, `one_line`, `mr_count`, `commit_count`
- 四個固定 heading（順序）：`## 本週重點`、`## 成長面向`、`## 待確認`、`## 已釐清`
- 各 section 使用 `- ` bullet；`待確認` 每條將由後端寫入 `pending_items`；`已釐清` 每條（須為既有 open 原文）由後端 ingest 自動 resolve

`project` 必須等於 `manifest.project_name`。`date` 必須等於 `manifest.run_date`。

---

## 4. 更新長期檔案（每位有產報告的工程師）

每位工程師產完 `{report_root}/{person}/{run_date}/summary.md` 後，**依序**更新專案層與人物層長期檔。兩層 `YYYY-MM.md` 語意不同，**皆須維護**。

### 4.1 專案層（本 repo 月度成長）

路徑：`{report_root}/{person}/`

| 檔案 | 動作 |
|------|------|
| `YYYY-MM.md` | **必做**。以 `run_date` 的 `YYYY-MM` 為檔名；追加本週在**本專案**的成長段落（技術深度、交付、review 互動等；可引用 summary 的 `## 成長面向`，但須改寫為月度累積敘事，非整段複製） |
| `index.md` | （可選）追加本專案技術脈絡段落 |

專案層月檔記錄「這個人在這個 repo 當月長大了什麼」，供單專案深讀與人物層綜合的素材來源。

### 4.2 人物層（跨專案綜合；趨勢 Tab 主資料源）

路徑：`{person_report_root}/{display_name}/`

| 檔案 | 動作 |
|------|------|
| `index.md` | 追加或修訂「長期觀察」段落（跨專案綜合敘事；引文式、累積；非每週重複貼 summary） |
| `YYYY-MM.md` | **必做**。以 `run_date` 的 `YYYY-MM` 為檔名；追加本週**跨專案**成長綜合段落（引用各專案本週重點，非複製專案層月檔全文） |
| `_notes.md` | 將本次 `## 待確認` 條目追加為 `- [YYYY-MM] 問題文字`（供趨勢「歷史待確認」） |

人物層月檔記錄「這個人整體當月的成長軌跡」，由趨勢 API `growth_timeline` 讀取。

若檔案不存在，建立並寫入標題與首段。若已存在，**追加**新段落，保留舊內容。

---

## 5. 品質與風格

- **語言**：繁體中文（技術名詞、程式識別符可保留英文）。
- **one_line**：一兩句話，管理者掃描用；含本週主軸 + 是否有待確認。
- **本週重點**：2–5 條，具體、可帶入 1on1 討論。
- **成長面向**：1–3 條，建設性、非評判式。
- **待確認**：0–5 條，**問句形式**，供管理者當面釐清；避免已能在 repo 內定論的事實題。遵守下方「待確認延續規則」。
- **禁止**：要求使用者輸入、輸出「請確認是否寫入」、執行 `glab mr note` / merge、修改 SQLite。

### 待確認延續規則（硬性）

寫每位 `{person}` 的 `## 待確認` / `## 已釐清` 前，從 `manifest.open_pending` 篩出 `display_name`（或 `person_id`）對應此人的條目。對每一條 open 議題，**三選一**：

1. **延續中**：本週仍相關 → 原文寫入 `## 待確認`（**禁止**同義改寫）。
2. **已釐清**：本週確認已解決 → 原文寫入 `## 已釐清`，且**不得**再出現在 `## 待確認`。後端 ingest 會將匹配的 open 列標為 resolved（workflow 本身仍不寫 SQLite）。
3. **省略**：仍 open 但本週不提 → 兩區都不寫該句；DB 保持 open（不自動 resolve）。

**全新議題**：僅在沒有對應 `open_pending` 條目時，才可用新措辭新增到 `## 待確認`（仍受 0–5 條上限）。不得把新問題寫進 `## 已釐清`。

---

## 6. 結束

- 所有 `{person}` 處理完畢後正常 exit。
- 不要輸出 session 摘要給人類；產物以檔案為準。
- 若單一 `{person}` 因資料不足只能產出簡短報告，仍須寫符合契約的 `summary.md`（`待確認` 可為空列表）。

---

## 7. 快速檢查清單

- [ ] 已 Read manifest.json
- [ ] 已依 manifest `authors` 處理每位工程師（非自行 git 歸戶）
- [ ] 已讀 `open_pending`；延續／已釐清沿用原文；未把已釐清項同時寫進待確認
- [ ] 每份 `summary.md` frontmatter 與四個 heading 正確（含可空的 `## 已釐清`）
- [ ] 路徑均在 `{report_root}/{person}/{run_date}/`
- [ ] 已更新 `{report_root}/{person}/` 下 `YYYY-MM.md`（若有產 report）
- [ ] 已更新 `{person_report_root}/{display_name}/` 下 `index.md` / `YYYY-MM.md` / `_notes.md`（若有產 report）
- [ ] 未 Write repo 原始碼、未互動詢問
