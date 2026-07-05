## Why

MR review 只拿 diff 常缺上下文（相依檔案、完整型別解析），導致 reviewer 判斷失準。要讓 reviewer 拿到「可操作的檔案樹」，就必須把目標分支實際 checkout 到本地；同時避免在 `repo_path` 直接放一份 working copy，改用單一 bare repo 承載多個分支的 git worktree。

## What Changes

- **BREAKING**：`repo_path` 語意從「已 checkout 的 working copy」改為「bare + worktree 容器目錄」。舊的「repo_path 直接是 git working copy」假設廢除。
- Server 啟動載入 `projects.yaml` 時，對每個 project 冪等建立 `.bare/`（`git clone --bare`）與各常駐分支的 worktree（方案 A：預先 provision，非熱路徑 lazy clone）。
- `projects.yaml` schema 變更：`git_remote_url` 改為必填；新增 `default_branches`（清單，允許多個常駐分支）。
- 每次 review 前對目標 worktree 針對性 `fetch` 單一 ref 並 `reset --hard` 對齊遠端（source branch 常 force-push）。
- MR worktree 以「轉義後 source branch 名 + short hash」命名，避免 `feature/x` 與 `feature-x` 路徑碰撞；同一分支多 MR 天然共用一個 worktree。
- 錯誤隔離：clone / fetch / 磁碟不足 / 分支被刪 各有明確處置（標記 project unhealthy、retry、skip、必要時 remove worktree），任一失敗不使 server crash、不影響其他 project。
- 同一 repo 的 worktree 與 fetch 操作 per-repo 序列化（共享物件庫），不同 repo 可並行。
- Reviewer 執行的工作目錄從 `projects.repo_path` 改為目標 worktree 路徑（weekly batch 指常駐 worktree，MR review 指該 MR worktree）。

## Capabilities

### New Capabilities

- `repo-worktree`: bare clone 與 git worktree 的 provision、更新（fetch/reset）、命名、錯誤隔離與 per-repo 並發控制。

### Modified Capabilities

- `project-config`: `git_remote_url` 由選填改必填；新增 `default_branches` 清單欄位；`repo_path` 解析後語意改為 bare+worktree 容器；git 偵測改為 provision 前置。
- `reviewer-execution`: worker 子行程工作目錄由 `projects.repo_path` 改為解析出的目標 worktree 路徑。

## Impact

- Affected specs: `repo-worktree`（新增）、`project-config`（修改）、`reviewer-execution`（修改）
- Affected code:
  - New:
    - backend/src/worktree.rs
  - Modified:
    - backend/src/executor.rs
    - backend/src/projects.rs
    - backend/src/config.rs
    - backend/src/lib.rs
    - projects.yaml
    - .env.example
    - README.md
  - Removed: (none)
