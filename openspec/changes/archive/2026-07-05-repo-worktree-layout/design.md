## Context

現況：`projects.rs` 的 `load_from_yaml` 讀取 `projects.yaml`，用 `resolve_repo_path` 把 `repo_path` 解析為 `{DATA_ROOT_DIR}/repos/{slug}`，並以 `detect_git` 偵測該路徑是否為既有 git working copy。`executor.rs` 的 `build_command` 直接把子行程 `current_dir` 設為 `projects.repo_path`，假設該路徑已是一份可讀的 checkout。MVP 不 auto-clone。

MR review 若只取 diff，會缺少相依檔案與完整型別上下文，導致 reviewer 判斷失準。需求是給 reviewer 一份「可操作的檔案樹」，且不在 `repo_path` 直接放單一 working copy，改用一個 bare repo 承載多分支的 git worktree。

決策已於 grill 階段全數敲定（決策 A–H），本文件記錄其技術理由與實作契約。

## Goals / Non-Goals

**Goals:**

- `repo_path` 成為「bare + worktree 容器目錄」，內含 `.bare/` 與多個 worktree。
- Server 啟動時冪等 provision 每個 project 的 `.bare/` 與常駐分支 worktree（方案 A）。
- 提供依 branch 取得（必要時建立/更新）worktree 路徑的介面，供 reviewer 執行使用。
- clone / fetch / 磁碟不足 / 分支被刪 的錯誤隔離：不 crash、不跨 project 汙染。
- 同一 repo 的 worktree 與 fetch 操作序列化；不同 repo 並行。

**Non-Goals:**

- MR merge 後的 worktree 自動回收、常駐 worktree 的 GC — 延後至未來 change。唯一自動清理為「source branch 遠端已刪 → remove 該 worktree」。
- 改用 libgit2 — 沿用系統 `git` CLI。
- 變更 reviewer 對 source tree 的寫入約束 — 仍只限 manifest 內路徑。
- MR polling 的觸發與排程 — 屬其他 capability，本 change 只提供 worktree 供給介面。

## Decisions

### 啟動時預先 provision bare 與常駐 worktree（方案 A）

在載入 `projects.yaml` 後，對每個 project 冪等執行：`.bare/` 不存在則 `git clone --bare <git_remote_url> .bare`；每個 `default_branches` 分支的 worktree 不存在則建立。理由：clone 大 repo 可能數分鐘，放在 MR review 熱路徑（lazy clone）會撞 timeout；預先做且冪等可讓失敗只影響單一 project。替代方案 lazy clone、手動 CLI 皆被否決（延遲爆掉 / 多一道人工步驟）。

### bare + worktree 目錄佈局與 fetch refspec

佈局為 `<repo_path>/.bare/`、`<repo_path>/.git`（內容 `gitdir: ./.bare`，方便容器目錄直接跑 git）、每個常駐分支與 MR 各一個 worktree 子目錄。`git clone --bare` 預設 refspec 不含 `+refs/heads/*:refs/remotes/origin/*`，clone 後必須顯式設定該 fetch refspec，否則後續 `fetch` 抓不到分支。

### projects.yaml schema 變更：git_remote_url 必填、新增 default_branches 清單

`git_remote_url` 由選填改必填（方案 A 需 URL 才能 clone；缺 URL 的 entry 視為設定錯誤並標記該 project unhealthy）。新增 `default_branches`（YAML 清單），承載常駐 worktree 分支集合，允許多個（如 `main` 與 `develop` 並存），多數只填一個。此欄位獨立於既有 `projects.default_branch`（偵測用單值），命名區隔避免混淆。

### MR worktree 命名以轉義 branch 名加 short hash 去碰撞

worktree 目錄鍵 = `轉義(source_branch)` + `-` + `short_hash(source_branch)`。轉義規則：`/` 及任何非 `[A-Za-z0-9._-]` 字元一律轉 `-`。純轉義會使 `feature/x` 與 `feature-x` 撞名，附加 source branch 全名的 short hash（如 SHA-1 前 8 碼）保證唯一並保留可讀前段。常駐 worktree 集合小且需人看得懂，使用純轉義名不加 hash。同一 source branch 的多個 MR 因鍵相同而共用一個 worktree。

### review 前針對性 fetch 單一 ref 並 reset --hard 對齊遠端

每次 review 前對目標 worktree 執行 `git fetch origin <branch>`（單一 ref，省流量）後 `git reset --hard origin/<branch>`。source branch 常 force-push，硬對齊遠端才能拿到正確樹。前提：worktree 對 reviewer 純唯讀，可隨時被硬重置，故無本地變動遺失風險。

### 錯誤隔離：clone/fetch/磁碟/分支被刪 各自處置且不 crash

- clone（bare 初始化）失敗：標記該 project unhealthy，server 繼續其他 project，不 crash。
- fetch 暫時性失敗（網路/timeout）：retry 3 次指數退避；仍失敗則該次 review 標 failed、保留舊 worktree 不動，不影響其他 MR。
- source branch 遠端已刪（fetch 回報 ref 不存在）：判定 MR 無效 → `git worktree remove` 該 worktree、skip review。此為現階段唯一自動清理。
- 磁碟空間不足：clone / worktree add 前檢查可用空間低於門檻（可設，預設 2GB）即拒絕該操作、標 project unhealthy 告警，不 crash。

### per-repo 序列化並發控制

同一 `.bare/` 為共享物件庫，`worktree add` 與 `fetch` 對同 repo 加鎖（以 repo_path 為 key 的 `tokio::Mutex` map）；不同 repo 可並行。避免並發 worktree 操作破壞物件庫或 worktree 註冊表。

### reviewer 執行工作目錄改指向目標 worktree

`executor.rs` 的 `build_command` 將 `current_dir` 與 `--add-dir` 由 `projects.repo_path` 改為解析出的目標 worktree 路徑：weekly batch 指常駐 worktree（取 `default_branches` 首項），MR review 指該 MR 的 worktree。reviewer 對 worktree 唯讀，寫入仍只限 manifest 內路徑。

## Implementation Contract

**行為（Behavior）：**

- Server 啟動載入 `projects.yaml` 後，對每個具備 `git_remote_url` 的 project：`<repo_path>/.bare/` 被建立（若不存在）、每個 `default_branches` 分支在 `<repo_path>/<轉義分支名>/` 有一個 checkout 出的 worktree。重跑不重複 clone（冪等）。
- 提供一個供給函式：輸入 (project, branch)，輸出該 branch 的 worktree 絕對路徑；不存在則建立，存在則 `fetch` + `reset --hard` 更新後回傳。
- reviewer 子行程的工作目錄為上述供給函式回傳的 worktree 路徑，而非 `repo_path` 本身。

**介面 / 資料形狀（Interface / data shape）：**

- `projects.yaml` 每個 entry：`name`（必填）、`repo_path`（必填）、`git_remote_url`（必填）、`default_branches`（分支字串清單，至少一項）。
- 新模組 `backend/src/worktree.rs` 匯出：
  - provision 函式：對單一 project 冪等建立 `.bare/` 與常駐 worktree，回傳成功或 unhealthy 原因。
  - worktree 供給函式：`(repo_path, branch) -> Result<PathBuf>`，含 fetch/reset 更新。
  - branch → 目錄名轉換函式：常駐（純轉義）與 MR（轉義+short hash）兩種鍵，行為可單元測試。
- per-repo 鎖：以 repo_path 為 key 的鎖表，供給/provision 期間持有。

**失敗模式（Failure modes）：**

- clone 失敗、磁碟不足、缺 `git_remote_url`：project 標 unhealthy，記錄原因，不 panic、不中止其他 project。
- fetch 暫時失敗：retry 3 次後回 Err，呼叫端（reviewer 執行）將該次 review 標 failed，worktree 維持原狀。
- 分支遠端已刪：供給函式偵測後 remove worktree 並回明確 Err（分支不存在），呼叫端 skip。

**驗收條件（Acceptance criteria）：**

- 單元測試：branch → 目錄名轉換對 `feature/x` 與 `feature-x` 產生不同目錄；轉義只保留 `[A-Za-z0-9._-]`。
- 單元測試：`resolve_repo_path` 既有行為不回歸。
- 整合測試：對一個本地 bare fixture，provision 建出 `.bare/` 與常駐 worktree；供給函式對新 branch 建 worktree、對既有 branch fetch+reset 更新；分支刪除情境會 remove worktree 並回 Err。
- 整合測試：缺 `git_remote_url` 或 clone 失敗時 project 標 unhealthy 且其他 project 仍正常 provision。
- `executor.rs` 測試：子行程 `current_dir` 為供給函式回傳的 worktree 路徑。

**範圍邊界（Scope boundaries）：**

- In scope：`.bare` + worktree 的 provision / 更新 / 命名 / 錯誤隔離 / per-repo 並發、`projects.yaml` schema、executor 工作目錄切換、README 與 `.env.example` 對應更新。
- Out of scope：merge 後回收與 GC、MR polling 觸發邏輯、libgit2、reviewer 寫入約束變更。

## Risks / Trade-offs

- [`git clone --bare` 預設 refspec 抓不到分支] → clone 後顯式設定 `+refs/heads/*:refs/remotes/origin/*`，並以整合測試覆蓋 fetch 分支路徑。
- [多常駐分支與大量 MR worktree 累積磁碟] → 本 change 以磁碟門檻檢查止血並標 unhealthy；完整 GC 延後，於 proposal Non-Goals 明記，避免誤以為已涵蓋。
- [`reset --hard` 誤丟本地變動] → 前提是 worktree 純唯讀；reviewer 寫入約束不變（僅 manifest 路徑），故 source tree 無本地變動。若未來 reviewer 需寫 source tree，此前提需重新評估。
- [per-repo 鎖使同 repo 的多 MR 供給序列化，可能拖慢] → 可接受：物件庫安全優先於併發；不同 repo 仍並行。
- [Windows 路徑分隔與 `.git` gitdir 檔可攜性] → 目錄名轉義已排除非法字元；`.git` 檔用相對 `gitdir: ./.bare` 保持可攜。

## Migration Plan

- 既有 `projects.yaml` 需補上 `git_remote_url`（若缺）與 `default_branches`；缺 `git_remote_url` 的 project 啟動後標 unhealthy 而非 crash，可漸進修正。
- 舊語意（`repo_path` 為既有 working copy）不再支援；部署前需確保 `repo_path` 指向可寫的容器目錄（將由 provision 填入 `.bare/` 與 worktree）。
- 回滾：還原 `executor.rs` 工作目錄為 `repo_path` 並移除 provision 呼叫即可回到 MVP 行為；`.bare/` 與 worktree 目錄殘留無害，可手動刪除。

## Open Questions

- （無）決策已於 grill 階段全數敲定。
