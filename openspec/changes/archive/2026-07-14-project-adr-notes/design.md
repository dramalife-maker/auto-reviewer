## Context

軌道 1（週報）與軌道 2（MR review）都會對「技術選擇／為何這樣做」產生追問：MR 草稿有「建議追問」，週報有 `## 待確認`。技術選擇若已在 MR agent chat 釐清，合併進常駐分支後週報仍可能當成新議題；目前只有 **person** 層 `_notes.md`／`pending_items`（1on1 待確認），沒有 **project** 層決策記憶。

現況約束：
- 週報／MR headless 只經 manifest 路徑寫檔，不查 SQLite
- Agent chat 為 `--resume` 短回合，目前不附加 skill 檔，但 Claude 已有 `Write` + `--add-dir` data root
- `reports/_people/` 用底線前綴避開專案名；`reports/{project}/` 下已有 `_pending` 等人／產物慣例

## Goals / Non-Goals

**Goals:**
- 專案 ADR 落在 `reports/{project}/.notes/`：`index.md` 索引 + `adr-YYYYMMDD-slug.md` 正文
- Agent chat **僅在**管理者顯式指令時依 skill 寫入
- 週報與 MR scan **強制**先讀 index，已知決策不得再問／不得當新 `待確認`
- Manifest（與 agent-turn）暴露可解析的 `notes_dir`

**Non-Goals:**
- 自動／混合草稿寫入 ADR（日後依使用情況再議）
- 週報 `## 已釐清` 或 pending resolve 自動升格為 ADR
- 把 ADR commit 進業務 repo
- UI 瀏覽／編輯 ADR
- 語意近似去重（v1 靠 agent 讀全文判斷；不做 embedding）

## Decisions

### ADR 目錄採 index + 單則檔（非單檔全文）

`index.md` 只列一覽表列連到 `adr-YYYYMMDD-slug.md`。理由：chat 與未來其他寫入源並發時，單檔全文易互相覆蓋；索引更新與正文新增職責分離。

替代（否決）：全文都寫在 `index.md`——實作短但膨脹與並發衝突風險高。

### 寫入觸發僅顯式指令（v1）

管理者在 agent chat 明確說「記成 ADR」「寫入決策」等 skill 白名單語句才落檔。理由：與「省略 ≠ resolve」同一原則；chat 有人在線，顯式成本低。

替代（暫緩）：自動判斷收斂／混合草稿——等使用情況再考慮變混合。

### `notes_dir` 由後端寫入 manifest 與 agent-turn 上下文

路徑固定為 `{DATA_ROOT_DIR}/reports/{project_name}/.notes`（字串正規化與既有 `report_root` 相同）。週報／MR poll manifest 新增必填欄位 `notes_dir`。Agent-turn 在 resume 訊息前注入同一路徑，並附加 `skills/project-adr-notes/` 契約檔。

替代（否決）：只靠 agent 自己用 `report_root` 推路徑——resume session 未必記得／未讀過 manifest。

### Agent-turn 附加 ADR skill，不改 draft／發佈語意

Chat 寫 ADR **不**自動改 `draft_md_path`、不發佈 GitLab。Skill 成功後回覆宣告寫入的檔名；失敗則說明原因且不更新 index。

### 兩軌必讀與禁止重問寫在 workflow，不重建 pending 去重

已知決策的「禁止重問」是 agent 行為契約，不是 `pending_items` 字串去重延伸。`pending_items` 仍只服務人物待確認；ADR 是專案事實。

### `.notes` 為專案報告根下保留目錄名

`reports/{project}/.notes/` 不得被當成工程師 display_name 資料夾；不以 `.notes` 產出人物報告。與 `_pending`、`_people` 同屬保留前綴慣例（此處用 dot 符合「索引隱藏目錄」語感）。

### ADR 正文用 `<tl;dr>`，不用 YAML／`<meta>`

對齊 `dev:post-bug` learning-notes：H1 標題後**必須**有 `<tl;dr>...</tl;dr>`；掃描重點**全部**用 TL;DR 內固定粗體鍵，**禁止** YAML frontmatter，也**禁止**另開 `<meta>`。

**不**在 TL;DR（或檔內其他處）記錄 `date`／`status`／`source`／`mr_iid`——對「不要再問」無幫助；日期若要掃可用檔名 `adr-YYYYMMDD-…`，來源由 chat 當下情境即可。

固定鍵（順序建議）：
- **何時要想起這則：**
- **決策：**
- **不要做／不要再問：**
- **要做：**
- **意圖：**
- **自問（可選）：**

TL;DR 之後正文建議：`## 為何這樣選（意圖）`（對齊 post-bug「使用者意圖」）、`## Context`、`## Decision`、`## Consequences`（或繁中等價 heading）。

兩軌 agent **優先只讀** index 列＋各檔 `<tl;dr>`；需要細節再讀後續章節。

替代（否決）：YAML frontmatter——與專案 learning-notes 慣例不一致，且多一套語法。  
替代（否決）：`<meta>` + `<tl;dr>`——欄位與掃描重點分兩塊，多一層無必要。  
替代（否決）：在 TL;DR 塞 date／status／source／mr_iid——增加寫入噪音，讀側不需要。

### index 更新規則對齊 post-bug（禁止整檔覆寫）

寫入前**必須** Read 現有 `index.md`；**只新增一列**；僅當檔案不存在時才用初始表頭範本建立。不得用空白表或範本整檔覆寫。

## Implementation Contract

**Behavior**
- 管理者在 draft MR 的 agent chat 下顯式指令後，agent 在 `notes_dir` 新增一則 ADR 檔，並在 `index.md` 追加一列；回覆含檔名
- 週報／MR headless 開場讀 `{notes_dir}/index.md`（缺檔＝尚無決策）；寫 `待確認`／建議追問前不得重複已知決策主題
- 後端不解析 ADR 內容入庫（v1 純檔案契約）

**Interface / data shape**
- Manifest（weekly 與 mr_poll）新增：`notes_dir`（string，絕對或 data-root 下已正規化路徑，與 `report_root` 風格一致）
- 目錄：
  - `{notes_dir}/index.md` — Markdown 表格索引（Date｜File｜摘要；可空表／可缺檔）；更新僅追加列；Date 取自檔名 `adr-YYYYMMDD-…`，不寫進 TL;DR
  - `{notes_dir}/adr-YYYYMMDD-slug.md` — `#` 標題 + `<tl;dr>`（僅掃描用粗體鍵，無 YAML／無 date·status·source·mr_iid）+ 意圖／Context／Decision／Consequences 章節
- Agent-turn：附加 `skills/project-adr-notes/` 下契約；上下文含 `notes_dir=<path>`
- Explicit trigger phrases（skill 白名單，至少含中文「記成 ADR」「寫入決策」與英文 `record as ADR`）

**Failure modes**
- 缺 `index.md`：讀側視為空；寫側建立目錄與 index
- 非顯式指令：MUST NOT 寫 ADR
- 寫入失敗：回覆錯誤；不得留下「index 有列但檔不存在」的半套（先寫 ADR 檔再改 index；index 失敗則回覆警告並指出孤兒檔需手動修）

**Acceptance criteria**
- Unit／integration：manifest JSON 含 `notes_dir` 指向 `reports/{project}/.notes`
- Executor／command builder：agent-turn 含 ADR skill 路徑（測試可 assert argv 或 fixture prompt）
- Workflow／contract 檔含「必讀 notes」與「禁止重問」可 grep 的硬性條款
- Fixture：給定 index 一則決策，workflow 檢查清單明示不得將同主題列入新 `待確認`

**Scope boundaries**
- In：檔案布局、skill、manifest、兩軌／chat 讀寫行為契約、必要測試
- Out：後端 ADR CRUD API、前端 UI、自動升格、業務 repo 同步

## Risks / Trade-offs

- [Agent 忽略顯式規則仍亂寫] → Mitigation：skill 白名單短、回覆必須宣告路徑；日後可加混合草稿
- [Agent 未讀 index 仍重問] → Mitigation：workflow checklist 硬性條款 + manifest 路徑醒目；無法 100% 保證 LLM 遵守
- [index／正文並發損壞] → Mitigation：v1 僅 chat 顯式寫、實務上單管理者；檔案級無鎖
- [`.notes` 與未來隱藏掃描] → Mitigation：spec 明訂保留名；掃描人物目錄時跳過 dot 目錄

## Migration Plan

- 既有專案：無 `.notes` 即可；首次讀視為空、首次寫建立
- 無需 DB migration
- Rollback：移除 skill／manifest 欄位與 workflow 條款；磁碟上 `.notes` 可留可刪

## Open Questions

- （無阻塞）顯式觸發白名單精確用詞可在 apply 時由 skill 定稿

