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
   - `run_date`（本次報告日期，`YYYY-MM-DD`）
   - `since`（分析窗口起日，`YYYY-MM-DD`，含當日）
   - `output_contract`（固定為 `output-contract.md`，格式見同目錄 appended 規格）
3. 若 `mode` 不是 `weekly_batch`，停止並在 stderr 說明（仍 exit 0 除非無法讀 manifest）。

**寫入限制（硬性）**：

- 允許寫入：`{report_root}/**` 底下所有檔案。
- 允許讀取：`repo_path` 下 git 歷史、`report_root` 既有檔案（含 `_pending/`）。
- **禁止**寫入 manifest 路徑以外的新根目錄、禁止修改 `repos/` 原始碼（除 Read 外不 Write repo 內容）。

---

## 1. 盤點本週活躍工程師

在 `repo_path` 用 git 找出 `since` 至 `run_date` 有 commit 的作者：

```bash
git log --since="${since}T00:00:00" --until="${run_date}T23:59:59" \
  --format='%an|%ae' --no-merges
```

規則：

- 以 **display name**（`%an`）作為 `person` 目錄名與 summary frontmatter 的 `person` 欄位。
- 同一 `%ae` 若對應多個 `%an`，合併為同一 `person`（取最常見的 `%an`）。
- **略過** merge commit 本身；若僅 merge、無實質 commit，可不產報告。
- 若窗口內 **無任何作者**，寫完 manifest 後正常結束（不視為錯誤）。

統計每位作者的 `commit_count`（窗口內非 merge commit 數）。`mr_count` 若無法從 git 可靠取得，填 `0` 或省略（後端接受 null）。

---

## 2. 每位工程師：收集素材

對每位 active author（以下稱 `{person}`）：

### 2.1 Git 活動

- `git log` / `git show` / `git diff` 理解本週變更主題、技術深度、review 互動（若 log 可見）。
- 聚焦 **行為與成長**，非產量排名；避免「commit 多＝表現好」這類結論。

### 2.2 MR 觀察片段（可選）

若存在 `{report_root}/{person}/_pending/` 下 `.md` 片段（軌道 2 預留）：

- Read 全部片段，合併進本週分析。
- 寫完本次報告後，將已消費的片段**搬移**至 `{report_root}/{person}/_pending/_archived/`（若目錄不存在則建立），不要刪除。

### 2.3 歷史脈絡（可選）

Read 若已存在：

- `{report_root}/{person}/index.md` — 長期觀察
- `{report_root}/{person}/YYYY-MM.md`（`run_date` 所在月份）
- `{report_root}/{person}/_notes.md` — 歷史待確認

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
- 三個固定 heading：`## 本週重點`、`## 成長面向`、`## 待確認`
- 各 section 使用 `- ` bullet；`待確認` 每條將由後端寫入 `pending_items`

`project` 必須等於 `manifest.project_name`。`date` 必須等於 `manifest.run_date`。

---

## 4. 更新長期檔案（每位有產報告的工程師）

路徑均在 `{report_root}/{person}/`：

| 檔案 | 動作 |
|------|------|
| `index.md` | 追加或修訂「長期觀察」段落（引文式、累積敘事；非每週重複貼 summary） |
| `YYYY-MM.md` | 以 `run_date` 的 `YYYY-MM` 為檔名；追加本週摘要段落，供趨勢「成長軌跡」 |
| `_notes.md` | 將本次 `## 待確認` 條目追加為 `- [YYYY-MM] 問題文字`（供趨勢「歷史待確認」） |

若檔案不存在，建立並寫入標題與首段。若已存在，**追加**新段落，保留舊內容。

---

## 5. 品質與風格

- **語言**：繁體中文（技術名詞、程式識別符可保留英文）。
- **one_line**：一兩句話，管理者掃描用；含本週主軸 + 是否有待確認。
- **本週重點**：2–5 條，具體、可帶入 1on1 討論。
- **成長面向**：1–3 條，建設性、非評判式。
- **待確認**：0–5 條，**問句形式**，供管理者當面釐清；避免已能在 repo 內定論的事實題。
- **禁止**：要求使用者輸入、輸出「請確認是否寫入」、執行 `glab mr note` / merge、修改 SQLite。

---

## 6. 結束

- 所有 `{person}` 處理完畢後正常 exit。
- 不要輸出 session 摘要給人類；產物以檔案為準。
- 若單一 `{person}` 因資料不足只能產出簡短報告，仍須寫符合契約的 `summary.md`（`待確認` 可為空列表）。

---

## 7. 快速檢查清單

- [ ] 已 Read manifest.json
- [ ] 每份 `summary.md` frontmatter 與三個 heading 正確
- [ ] 路徑均在 `{report_root}/{person}/{run_date}/`
- [ ] 已更新 `index.md` / `YYYY-MM.md` / `_notes.md`（若有產 report）
- [ ] 未 Write repo 原始碼、未互動詢問
