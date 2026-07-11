## Context

`person-identity` 已提供建立人員、綁定／列出 identity、未歸戶佇列與前端未歸戶面板。`docs/idea/spec.md` §5 定義獨立「人員設定」頁（顯示名、多 identity、參與專案唯讀），但前端只有報告閱讀器左欄與未歸戶捷徑，沒有完整設定 UI。亦缺少更名、解除綁定、以及含參與專案的人員詳情 API。`participation` 表已在 schema／migration 中，專案列表會讀取；人員側尚未暴露。

## Goals / Non-Goals

**Goals:**

- 提供獨立人員設定視圖，完成建立、更名、identity 增刪、參與專案唯讀檢視。
- 更名時同步人物層 `reports/_people/{display_name}/` 目錄名。
- UI 支援 `git_email`、`gitlab_user`、`glab_user`；嚴格跨人 identity 衝突。
- 保留未歸戶面板。

**Non-Goals:**

- 刪除或封存人員、合併兩人。
- 遷移專案層 `reports/{project}/{old_display_name}/` 或改寫歷史 `summary.md` frontmatter。
- 即時 `git log` 推導參與專案。
- 認證／操作者歸屬。
- 待確認閉環、排程、執行紀錄（其他 change）。

## Decisions

### 獨立 AppView，不與報告閱讀器左欄合併

- **選擇**：新增 `AppView = 'people'`，版面比照專案設定（左清單 + 右表單）。報告閱讀器左欄仍只負責選人看報告。
- **理由**：設定與閱讀職責分離，避免在報告流裡塞表單。
- **替代**：只在未歸戶面板擴充 — 無法預先註冊、無法管理已綁 identity。

### 更名：DB + 僅 `_people/` 目錄；專案層報告不搬

- **選擇**：`PATCH` 更新 `people.display_name`；若 `reports/_people/{old}` 存在，rename 為 `{new}`。目標目錄已存在 → 409 且不改 DB。Rename 失敗則回滾 DB 顯示名。不搬專案層報告目錄、不改舊 summary frontmatter。
- **理由**：趨勢／長期檔以 `_people/{display_name}` 為準；專案層歷史路徑搬遷成本高且易漏。
- **替代**：級聯搬所有報告目錄 — 範圍過大，另開 change。

### 參與專案：reports ∪ participation，不掃 git

- **選擇**：`GET /api/people/{id}` 的 `projects` 為去重專案名，來源 `reports.person_id` 與 `participation.person_id` 的聯集。
- **理由**：夠用、快、與既有物化資料一致。
- **替代**：每次打開設定頁掃各 repo — 慢且 fragile。

### Identity 解除綁定允許刪到零

- **選擇**：`DELETE` 可刪除該人最後一個 identity；之後 commit 進未歸戶。跨人衝突與正規化規則沿用既有 bind 邏輯。
- **理由**：錯誤綁定必須能清掉；不強制至少一筆 email。
- **替代**：禁止刪最後一筆 — 卡住糾錯。

### 同人重複 bind 維持 no-op

- **選擇**：同一人重複 `POST` 同一 `(kind,value)` 仍成功 no-op（既有行為）；UI 避免重複送出。
- **理由**：避免無意義的 breaking change。

## Implementation Contract

### Person detail and rename

- **Behavior**：管理者可開啟人員設定、檢視 identities 與參與專案、更改顯示名；更名後趨勢路徑改跟新名。
- **Interface**：
  - `GET /api/people/{id}` → `{ id, display_name, identities: [...], projects: [{ id, name }] }`
  - `PATCH /api/people/{id}` body `{ "display_name": "..." }` → 200 更新後物件（或至少含 id/display_name）
- **Failure modes**：未知 id → 404；空名 → 400；重名或 `_people/{new}` 已存在 → 409；rename 失敗 → 5xx 且 DB 回滾為舊名
- **Acceptance**：整合測試覆蓋成功更名＋目錄 rename、目標目錄衝突 409、重名 409
- **In scope**：上述 API、人員設定 UI、DELETE identity
- **Out of scope**：見 Non-Goals

### Delete identity

- **Behavior**：可從某人移除一筆 identity；列表不再顯示該筆。
- **Interface**：`DELETE /api/people/{id}/identities/{identity_id}` → 204
- **Failure modes**：人員或 identity 不存在、或 identity 不屬於該人 → 404
- **Acceptance**：`identity` 整合測試覆蓋成功刪除與 404

### People settings UI

- **Behavior**：側欄進入人員設定；可新增人員、編輯顯示名、以三種 kind 新增／刪除 identity、看到參與專案唯讀列表；未歸戶面板仍可用。
- **Acceptance**：`npm run build`；手動或 UI 流程斷言主要操作可完成

## Risks / Trade-offs

- [更名後專案層報告目錄仍用舊名] → 文件註明；閱讀器依 DB person_id／最新路徑，舊檔案仍在磁碟可手動搬。
- [刪光 identity 後週報無人產出] → 預期行為；未歸戶會再出現。
- [participation 可能空] → 僅靠 reports 聯集；無報告時顯示空狀態文案。

## Migration Plan

- 無 DB migration。
- 前後端一併部署。
- Rollback：還原二進位；已更名的目錄與 DB 不自動回滾。

## Open Questions

（無）

