## Why

人員與 identity 的建立／綁定 API 與「未歸戶」面板已存在，但缺少獨立的人員設定頁，管理者無法集中檢視／編輯顯示名、管理多種 identity（含 GitLab username）、或解除錯誤綁定。規格 §5 要求以人為中心一次設定到位；現在專案設定與 MR 收件匣已有對應 UI，人員設定是明顯缺口。

## What Changes

- 新增前端「人員設定」視圖：左欄人員清單 + 右側表單（顯示名、identity CRUD、參與專案唯讀）。
- 新增 `GET /api/people/{id}` 詳情（含 identities 與參與專案名）。
- 新增 `PATCH /api/people/{id}` 更名；同步 rename `reports/_people/{old}/` → `{new}/`（目標已存在則 409）；**不**刪除人員、**不**遷移專案層報告目錄。
- 新增 `DELETE /api/people/{id}/identities/{identity_id}` 解除綁定。
- Identity UI 支援 `git_email`、`gitlab_user`、`glab_user`；跨人衝突維持 409。
- 保留 header「未歸戶」面板作為捷徑。

## Capabilities

### New Capabilities

- `people-settings`: 人員設定頁 UI、人員詳情（含參與專案）、顯示名更名與人物層目錄同步。

### Modified Capabilities

- `person-identity`: 新增解除 identity 綁定 API；設定頁可管理多種 identity kind。

## Impact

- Affected specs: people-settings (new), person-identity (modified)
- Affected code:
  - New: (none required beyond existing modules; may extend `backend/src/identity.rs` helpers)
  - Modified: `backend/src/identity.rs`, `backend/src/server.rs`, `backend/src/error.rs`, `backend/tests/identity.rs`, `frontend/src/app.ts`, `frontend/src/api.ts`, `frontend/src/types.ts`, `frontend/src/style.css`, `docs/idea/schema.md`, `README.md`
  - Removed: (none)

