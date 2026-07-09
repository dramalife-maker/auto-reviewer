## Why

目前人員是由 `summary.md` frontmatter 的 `person` 顯示名稱自動 upsert，無法把同一真人的多個 Git 帳號歸到同一人。實際痛點包含：

- **同 repo 多帳號**：同一工程師用公司 email 與私人 email commit，現行會拆成多位人員。
- **跨平台多帳號**：GitLab 與 GitHub 上使用者名稱不同，但應對應同一位真人。
- **人員來源不明**：左欄人員目前僅在跑完 review 後才出現，且無法事前綁定帳號；`person_identities` / `unmatched_authors` 表已存在但程式未使用。

這與 `docs/idea/schema.md` 設計不一致，也導致週報分散、1on1 閱讀體驗破碎。

## What Changes

- 後端在執行 review 前，以 git author identity（優先 `git_email`）查詢 `person_identities` 決定 canonical `person_id`；未命中者寫入 `unmatched_authors`。
- 新增人員與身分管理 API：列出未歸戶 author、建立人員、綁定 identity（第一階段以 `git_email` 為主）。
- 定義管理者 onboarding 流程：可先預先建立人員並綁定 email，或 run 後從未歸戶佇列指認綁定，再重跑 review。
- 調整 reviewer-batch 執行契約：manifest 帶入後端解析後的 author→person 對照；workflow 依 canonical person 分組產出。
- 調整 summary ingestion：以 resolved `person_id` 寫入 `reports`；禁止僅靠 display name 自動建立新人員。
- 前端新增「未歸戶」提示與綁定 UI（建立新人員並綁定 / 綁定到既有人員）。
- 預留 `kind` 命名空間（`git_email`、`gitlab_user`、`github_user`）；本變更僅實作 `git_email` 自動歸戶。

## Non-Goals

- 人員合併（兩個 `people` 列合併、搬移歷史 reports）。
- `gitlab_user` / `github_user` 自動從 MR/PR 抓取與歸戶。
- 自動啟發式歸戶（同網域 email、模糊名稱比對）。

## Capabilities

### New Capabilities

- `person-identity`：人員 canonical 模型、identity 比對與未歸戶佇列、人員/身分綁定 API、執行前 author 解析、管理者 onboarding 流程。

### Modified Capabilities

- `reviewer-execution`：run 前 author 列舉與 manifest person 對照由後端產生；workflow 不再自行以 display name 歸戶。
- `report-reader`：ingestion 與 people API 行為改為基於 resolved `person_id`；人員列表可反映未歸戶狀態。

## Impact

- Affected specs: `person-identity`（新建）、`reviewer-execution`、`report-reader`
- Affected code:
  - New:
    - `backend/src/identity.rs`
    - `backend/tests/identity.rs`
    - `frontend/src/people.ts`（或同等身分管理 UI 模組）
  - Modified:
    - `backend/src/summary.rs`
    - `backend/src/reports.rs`
    - `backend/src/server.rs`
    - `backend/src/runs.rs`
    - `backend/src/worker.rs`
    - `skills/reviewer-batch/WORKFLOW.md`
    - `skills/reviewer-batch/output-contract.md`
    - `frontend/src/app.ts`
    - `frontend/src/api.ts`
    - `frontend/src/types.ts`
    - `README.md`（補充人員 onboarding 與未歸戶流程說明）


