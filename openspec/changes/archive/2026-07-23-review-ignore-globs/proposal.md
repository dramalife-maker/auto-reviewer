## Why

MR 素材產生時對變更檔案一視同仁，鎖檔（lock）、產生碼、vendor 目錄等雜訊檔會佔滿 change.diff 的位元組上限，把真正需要 review 的程式碼擠掉並觸發截斷標記。目前沒有任何管道能排除這類檔案，且設定若寫死在 YAML 或環境變數就必須重啟服務才能調整。

## What Changes

- 新增全域單列設定表 review_settings（id=1），以 JSON 陣列儲存 ignore_globs（git pathspec 字串清單），預設為空陣列。
- 新增 GET/PUT /api/review-settings 兩支端點；PUT 為全量覆蓋，並在寫入前正規化與驗證清單。
- MR 素材產生時，僅在產生 change.diff 的 git diff 指令附加 exclude pathspec；change.stat 與 change.log 不套用，保留被忽略檔案的檔名與規模可見性。
- 帶 pathspec 的 diff 指令失敗時降級：記錄警告後改以不帶 pathspec 重跑一次，避免單一錯誤 glob 讓整批 MR 靜默跳過。
- run manifest 新增 ignore_globs 欄位，並在 reviewer-batch 與 scan-mrs-headless 兩份 WORKFLOW 加入硬性指示，要求 agent 自行執行的 git 指令一律附帶 exclude pathspec（軟約束）。
- 前端 Dashboard 新增獨立的 Review 過濾卡片，以每行一條的文字框編輯清單並即時儲存，下一場 run 生效、無須重啟服務。

## Capabilities

### New Capabilities

- `review-ignore-globs`: 全域 review 檔案忽略清單的儲存、驗證、API 與前端編輯介面

### Modified Capabilities

- `reviewer-execution`: MR 變更素材產生時套用忽略清單並具備 pathspec 失敗降級行為

## Impact

- Affected specs: review-ignore-globs（新增）、reviewer-execution（修改）
- Affected code:
  - New:
    - backend/migrations/017_review_settings.sql
    - backend/src/review_settings.rs
  - Modified:
    - backend/src/lib.rs
    - backend/src/mr_change_materials.rs
    - backend/src/worker.rs
    - backend/src/server.rs
    - backend/src/runs.rs
    - frontend/src/api.ts
    - frontend/src/types.ts
    - frontend/src/pages/DashboardPage.tsx
    - skills/reviewer-batch/WORKFLOW.md
    - skills/scan-mrs-headless/WORKFLOW.md
  - Removed: （無）
