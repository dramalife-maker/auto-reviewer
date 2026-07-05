## 1. projects.yaml schema 與專案載入

- [x] 1.1 依「projects.yaml schema 變更：git_remote_url 必填、新增 default_branches 清單」，讓 YAML 每個 entry 解析出必填 `git_remote_url` 與非空 `default_branches` 清單；缺 `git_remote_url` 的 entry 仍儲存並標記 unhealthy。驗證：`backend/tests/project_config.rs` 新增測試斷言解析結果與缺 URL 時 project 被標 unhealthy 且其他 entry 正常載入。
- [x] 1.2 更新 `projects.yaml` 與 `.env.example` 範例，每個 project 具備 `git_remote_url` 與 `default_branches`。驗證：內容審查——範例可被 1.1 的載入器成功解析、欄位齊全。
- [x] 1.3 讓「Projects load from YAML at startup」保留既有 `resolve_repo_path` 三種解析行為，並將解析後 `repo_path` 語意記為 bare+worktree 容器目錄。驗證：`resolve_repo_path` 既有單元測試（slug/absolute/explicit-relative）不回歸。

## 2. worktree 模組：命名、佈局與並發

- [x] 2.1 依「MR worktree 命名以轉義 branch 名加 short hash 去碰撞」，於 `backend/src/worktree.rs` 實作 branch→目錄名轉換：常駐用純轉義名、MR 用轉義名+short hash，轉義只保留 `[A-Za-z0-9._-]`。驗證：單元測試斷言 `feature/x` 與 `feature-x` 產生不同 MR 目錄名，且轉義規則正確。
- [x] 2.2 依「bare + worktree 目錄佈局與 fetch refspec」，實作 bare clone 佈局建立：`git clone --bare` 後顯式設定 `+refs/heads/*:refs/remotes/origin/*` fetch refspec，並寫入容器目錄 `.git`（`gitdir: ./.bare`）。驗證：整合測試對本地 bare fixture 斷言 refspec 已設定、後續 fetch 能取得分支 head。
- [x] 2.3 依「per-repo 序列化並發控制」實作 "Worktree operations are serialized per repository"：以 repo_path 為 key 的 `tokio::Mutex` 表包住 worktree add/fetch/reset。驗證：整合測試斷言同 repo 兩個供給操作序列化、不同 repo 可並行。

## 3. provision 與 worktree 供給

- [x] 3.1 實作「Bare repository and resident worktrees are provisioned at startup」與「啟動時預先 provision bare 與常駐 worktree（方案 A）」：載入後對每個具 `git_remote_url` 的 project 冪等建立 `.bare/` 與各 `default_branches` 常駐 worktree。驗證：整合測試斷言首次 provision 建出 `.bare/` 與常駐 worktree、二次 provision 冪等不重 clone。
- [x] 3.2 實作「A worktree is supplied and updated on demand for a branch」與「review 前針對性 fetch 單一 ref 並 reset --hard 對齊遠端」：供給函式 `(repo_path, branch)->Result<PathBuf>`，不存在則建立、存在則 `git fetch origin <branch>` 後 `git reset --hard origin/<branch>`。驗證：整合測試斷言 force-push 後 worktree 被硬對齊到 `origin/<branch>`。
- [x] 3.3 讓「Worktree paths are derived from branch names without collision」在供給與 provision 路徑上被實際使用（常駐與 MR 兩種鍵）。驗證：整合測試斷言同一 source branch 的多次供給落在同一 MR 目錄。

## 4. 錯誤隔離

- [x] 4.1 實作「錯誤隔離：clone/fetch/磁碟/分支被刪 各自處置且不 crash」之 clone 與磁碟分支：clone 失敗、缺 `git_remote_url`、可用空間低於門檻（預設 2GB）時標 project unhealthy、記錄原因、不中止其他 project。驗證：整合測試斷言一個 project 失敗時其餘仍完成 provision 且進程不 panic。
- [x] 4.2 補齊 fetch 與分支被刪處置，並更新「Git repository detection updates project metadata」：fetch 暫時失敗 retry 3 次指數退避後回 Err 且保留舊 worktree；分支遠端已刪則 `git worktree remove` 並回 branch-gone Err；provision 成功時 `is_git_repo=1` 且 `default_branch` 取 `default_branches` 首項，失敗時 `is_git_repo=0`。驗證：整合測試涵蓋 retry 後 worktree 不變、刪除分支移除 worktree、metadata 欄位符合。

## 5. executor 整合

- [x] 5.1 依「reviewer 執行工作目錄改指向目標 worktree」更新「Worker executes reviewer skill subprocess per project」：`build_command` 的 `current_dir` 與 `--add-dir` 改用供給函式回傳的 worktree 路徑（weekly batch 取常駐首項，MR review 取該 MR worktree），worktree 無法供給時不啟動子行程並記錄失敗、續跑其他 project。驗證：`backend/tests/runs_execution.rs` 斷言子行程 `current_dir` 為 worktree 路徑、供給失敗時該 project 被 skip 而不影響其他。

## 6. 文件

- [x] 6.1 更新 `README.md` 說明 bare+worktree 佈局、`projects.yaml` 新欄位與 unhealthy 行為。驗證：內容審查——佈局圖與欄位說明與實作一致、無殘留舊「repo_path 即 working copy」敘述。
