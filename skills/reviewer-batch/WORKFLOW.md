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
- 若 repo 有 `CLAUDE.md`／`AGENTS.md`／`.claude/docs`，優先對照專案標準；否則可以通用框架當補充鏡片（scalability、SPOF／retry、idempotency、index／N+1、transaction 邊界、observability、決策理由是否充分）——**用來判斷交付品質，不是用來開長 checklist 塞進 summary**。

### 2.2 MR 觀察片段（可選）

軌道 2 會將**場次紀錄**寫入 `{report_root}/{person}/_pending/`（格式見 `skills/scan-mrs-headless/observation-guidelines.md`）。**僅** manifest 的 `published_pending_snippets` 所列路徑可折入週報（對應 `mr_reviews.status='published'`）；`draft` / `ignored` 的片段必須留在 `_pending/` 不動。

對每位 `{person}`：

1. 從 `published_pending_snippets` 篩出以 `{person}/_pending/` 開頭的 `.md` 路徑（相對於 `report_root`）。
2. Read 這些片段。依區塊消費（**改寫濃縮，禁止整段貼上**）：

| 觀察片段區塊 | 週報去向 |
|--------------|----------|
| 背景／提案重點／審查意見摘要 | `report.md`「技術與交付」或「協作與 review」；必要時 1 句進 summary「本週重點」 |
| **觀察到的思維模式** | summary「成長面向」與／或「本週重點」的行為訊號；`report.md` 可稍詳 |
| 後續追蹤中仍開放、適合 1on1 的問句 | 可成為新的 `## 待確認`（遵守延續規則；勿把已能在 repo 定論的事實題寫進去） |
| 上一輪已解／修法細節 | 最多一句帶過；不要把 ❌ 表複製進 summary |

3. 寫完本次報告後，**刪除**已消費的 `{report_root}/{person}/_pending/mr-*.md`（場次正文已在專案層 `YYYY-MM.md`；**不要**搬到 `_archived/`，也不需要 `_archived` 目錄）。
4. **不要** Read 或刪除未列於 `published_pending_snippets` 的 `_pending/` 檔案（含仍為 draft／ignored 的片段）。

### 2.3 歷史脈絡（可選）

Read 若已存在：

- `{person_report_root}/{display_name}/index.md` — **跨專案**長期觀察／思維模式（趨勢 Tab 主資料源；**稀疏**，見 §4.2）
- `{person_report_root}/{display_name}/YYYY-MM.md`（`run_date` 所在月份）— 跨專案月度成長
- `{person_report_root}/{display_name}/_notes.md` — **僅**歷史待確認（待確認清單用；**不是**長期思維模式檔）
- `{report_root}/{person}/index.md` —（可選）本專案技術脈絡補充
- `{report_root}/{person}/YYYY-MM.md`（`run_date` 所在月份）— **本專案**月度成長素材
- 同人專案層 `YYYY-MM.md`、`_pending/` 尚存片段 — 判斷「是否構成重複模式」時參考

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
（依 commit / 已發佈觀察的背景與審查摘要整理；可含模組、決策、trade-off）

## 協作與 review
（觀察片段的回應風格、清單內外品質分化等；可寫稍詳）

## 風險與待確認
（詳細版；summary 會精簡）

## 參考
- commit: <hash> — <subject>
- MR 觀察：已消費片段的相對路徑（若有）
```

### 3.2 `summary.md`（精簡版 — 硬性契約）

**必須**完全符合 appended `output-contract.md`：

- YAML frontmatter 含 `person`, `project`, `date`, `one_line`, `mr_count`, `commit_count`
- 四個固定 heading（順序）：`## 本週重點`、`## 成長面向`、`## 待確認`、`## 已釐清`
- 各 section 使用 `- ` bullet；`待確認` 每條將由後端寫入 `pending_items`；`已釐清` 每條（須為既有 open 原文）由後端 ingest 自動 resolve
- 有已發佈觀察時：「成長面向」優先吸收其**思維模式**（改寫濃縮）；勿貼場次全文或 ❌ 表

`project` 必須等於 `manifest.project_name`。`date` 必須等於 `manifest.run_date`。

---

## 4. 更新長期檔案（每位有產報告的工程師）

每位工程師產完 `{report_root}/{person}/{run_date}/summary.md` 後，**依序**更新專案層與人物層長期檔。兩層 `YYYY-MM.md` 語意不同，**皆須維護**。

**長期檔分工**：**流水帳進月檔；跨時間行為模式才進人物層 `index.md`。**  
`_notes.md` **只服務待確認歷史**（與 `index.md` 的思維模式**分檔**，勿把行為觀察寫進 `_notes.md`）。

### 4.1 專案層（本 repo 月度成長）

路徑：`{report_root}/{person}/`

| 檔案 | 動作 |
|------|------|
| `YYYY-MM.md` | **必做**。以 `run_date` 的 `YYYY-MM` 為檔名；追加本週在**本專案**的成長段落（技術深度、交付、review 互動；可引用 summary「成長面向」，改寫為月度累積敘事，非整段複製）。同月多段以 `---` 分隔。**注意**：MR 軌道可能已在同檔追加「架構審查」場次節——週報只追加成長綜合段，**不要**重貼已存在的 MR 場次全文 |
| `index.md` | （可選）僅在本專案出現**重複技術／協作模式**時追加；不要每週貼 summary |

專案層月檔記錄「這個人在這個 repo 當月長大了什麼」，供單專案深讀與人物層綜合的素材來源。

### 4.2 人物層（跨專案綜合；趨勢 Tab 主資料源）

路徑：`{person_report_root}/{display_name}/`

| 檔案 | 動作 |
|------|------|
| `YYYY-MM.md` | **必做**。追加本週**跨專案**成長綜合段落（引用各專案本週重點，非複製專案層月檔全文）；同月多段以 `---` 分隔 |
| `index.md` | **稀疏**。僅當本週觀察／已消費 MR 場次與過去（`index.md`、專案層月檔）構成**重複模式**或明確**成長跡象**時，才追加或修訂「長期觀察／思維模式」條目（標日期與 MR／專案作證據錨）。**單次現象不要每週改 index** |
| `_notes.md` | **僅**將本次 summary `## 待確認` 條目追加為 `- [YYYY-MM] 問題文字`（供趨勢「歷史待確認」）。**不要**把思維模式寫進 `_notes.md` |

建議 `index.md` 長期區塊形狀（可同義；初次建立時可寫標題）：

```markdown
# {display_name} 長期觀察

## 思維模式
- <跨場次重複的決策習慣或盲點>（初次／再確認：YYYY-MM；證據：專案／MR）

## 成長跡象
- <可驗證的改善>（YYYY-MM）

## 長期追蹤項目
- [ ] <跨週仍要跟的事項>
```

人物層月檔記錄「這個人整體當月的成長軌跡」，由趨勢 API `growth_timeline` 讀取。

若 `YYYY-MM.md` 不存在，建立並寫入標題與首段。若已存在，**追加**新段落，保留舊內容。`index.md` 不存在且本週無重複模式 → **可先不建立**，或只建標題、不寫假模式。

---

## 5. 品質與風格

- **語言**：繁體中文（技術名詞、程式識別符可保留英文）。
- **one_line**：一兩句話，管理者掃描用；含本週主軸 + 是否有待確認。
- **本週重點**：2–5 條，具體、可帶入 1on1；優先來自 git 主軸 + 已發佈觀察的濃縮，非產量列表。
- **成長面向**：1–3 條，建設性；優先吸收觀察片段的「思維模式」（改寫，不貼原文）。
- **待確認**：0–5 條，**問句形式**，供管理者當面釐清；避免已能在 repo 內定論的事實題。遵守下方「待確認延續規則」。
- **長期檔**：月檔每週可寫；人物層 `index.md` 僅重複模式／成長跡象才動；`_notes.md` 只追加待確認。
- **禁止**：要求使用者輸入、輸出「請確認是否寫入」、執行 `glab mr note` / merge、修改 SQLite；禁止把觀察場次全文或 MR 草稿 ❌ 表貼進 summary。

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
- [ ] 已消費之 `published_pending_snippets`：濃縮折入 report／summary（思維模式→成長面向）；未整段貼上；已**刪除**對應 `_pending` 檔（不建 `_archived`）
- [ ] 每份 `summary.md` frontmatter 與四個 heading 正確（含可空的 `## 已釐清`）
- [ ] 路徑均在 `{report_root}/{person}/{run_date}/`
- [ ] 已更新專案層與人物層 `YYYY-MM.md`（若有產 report）
- [ ] 人物層 `index.md`：僅重複模式／成長跡象才追加；未每週複貼 summary
- [ ] `_notes.md` 僅追加待確認文字（無思維模式）
- [ ] 未 Write repo 原始碼、未互動詢問
