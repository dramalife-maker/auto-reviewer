## 1. 身分解析核心模組（歸戶在後端、不在 LLM；情境 A：同 repo、多個 git 帳號；第一階段僅實作 `git_email` kind）

- [x] 1.1 實作 `backend/src/identity.rs`：`normalize_git_email` 與 `resolve_person_by_email`（第一階段僅 `git_email` kind），小寫 trim 後命中 `person_identities` 並回傳 canonical `display_name`；驗證：`backend/tests/identity.rs` **Git author email is normalized for identity lookup**
- [x] 1.2 實作 `record_unmatched_author`：未命中 identity 的 email 會 upsert `unmatched_authors` 並累計 `commit_count`；驗證：integration test 涵蓋 **Unmatched authors are recorded during run preparation**

## 2. Run 前 author 解析與 manifest.json 新欄位

- [x] 2.1 在 worker author 解析（run 前）於 worktree 列舉 authors，manifest 擴充 `authors` 陣列（僅已歸戶者）；驗證：`backend/tests/runs_execution.rs` **Weekly manifest includes resolved authors**
- [x] 2.2 落實未歸戶 author 不產報告、不建立 person：未歸戶者不進 manifest `authors`；驗證：fixture 一綁定一未綁，manifest 僅一筆

## 3. Identity 管理 API

- [x] 3.1 實作 `GET /api/unmatched-authors`，回傳含 `project_name` 的未歸戶列表；驗證：`backend/tests/identity.rs` **Unmatched authors list API**
- [x] 3.2 實作 `POST /api/people` 建立人員（重複 `display_name` 回 409）；驗證：**Create person API** 與 **Administrator can pre-register identities before review**
- [x] 3.3 實作 `POST /api/people/:id/identities` 綁定 identity，落實 `UNIQUE(kind, value)` 衝突處理（409）並清除 `unmatched_authors`；驗證：**Bind identity to person API**
- [x] 3.4 實作 `GET /api/people/:id/identities` 列出已綁 identity；驗證：**List identities for a person API**

## 4. Summary ingestion 變更與 People API

- [x] 4.1 調整 `summary.rs`：未知 person 跳過、不 INSERT（summary ingestion 變更）；驗證：**Summary files are parsed into reports and pending items** skip 情境
- [x] 4.2 調整 `GET /api/people` 回傳 `identity_count`；驗證：`backend/tests/report_reader.rs` **People list API exposes read and pending status** identity count 情境

## 5. Workflow 契約變更

- [x] 5.1 更新 `WORKFLOW.md`：workflow 契約變更為讀 manifest `authors`；驗證：**Reviewer-batch workflow uses manifest authors**
- [x] 5.2 更新 `skills/reviewer-batch/output-contract.md`：說明 `person` 必須等於 manifest `authors[].display_name`；驗證：契約文件與 manifest 形狀一致

## 6. 前端未歸戶管理（scope boundaries 內：綁定 UI，不含 person merge）

- [x] 6.1 新增 `fetchUnmatchedAuthors`、`createPerson`、`bindIdentity` API client 與 types；驗證：`npm run build` 通過
- [x] 6.2 header 顯示未歸戶數與綁定面板；驗證：**Web UI displays weekly reader and run controls** unmatched 情境與 **Frontend exposes unmatched author management**

## 7. 端對端驗證（情境 B：跨 Git host、不同使用者名）

- [x] 7.1 新增 integration test（情境 A/B：同 repo 多 email、跨 Git host 多 email 綁同一人）：兩份 summary 寫入同一 `person_id`；驗證：`cargo test -p reviewer-server`
- [x] 7.2 執行 `cargo test -p reviewer-server` 與 `cd frontend && npm run build` 全綠；驗證：CI 等價本地指令通過

## 8. 文件（情境 C：管理者如何「新增人員」）

- [x] 8.1 更新 `README.md`：說明人員 onboarding 兩條路徑（預先綁定 vs 未歸戶指認）與「未綁定不產週報」規則；驗證：人工審閱 README 涵蓋情境 C 流程






