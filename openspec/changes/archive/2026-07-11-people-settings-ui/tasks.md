## 1. Person detail and rename APIs

- [x] 1.1 實作 Person detail API：`GET /api/people/{id}` 回傳 `display_name`、`identities`、以及來自 `reports` ∪ `participation` 的參與專案列表；未知 id 回 404。對齊 design「參與專案：reports ∪ participation，不掃 git」與 spec「Person detail API includes identities and projects」。驗證：整合測試覆蓋有 identities／projects、空 projects、404。
- [x] 1.2 實作 Person display name rename：`PATCH /api/people/{id}` 更新顯示名；同步 rename `reports/_people/{old}/`（更名：DB + 僅 `_people/` 目錄；專案層報告不搬）；目標目錄已存在或重名回 409；rename 失敗回滾 DB。對齊 design「更名：DB + 僅 `_people/` 目錄；專案層報告不搬」與 spec「Person display name can be renamed」。驗證：整合測試覆蓋成功 rename、目錄衝突 409、重名 409。

## 2. Identity unbind and kind support

- [x] 2.1 實作 Delete identity／Unbind identity from person API：`DELETE /api/people/{id}/identities/{identity_id}` 回 204；錯人／不存在回 404；允許刪到零。對齊 design「Identity 解除綁定允許刪到零」「Delete identity」與 spec「Unbind identity from person API」。驗證：更新 `identity` 整合測試覆蓋成功、404、刪最後一筆。
- [x] 2.2 確認 bind 路徑支援 `gitlab_user`／`glab_user`（trim、不強制小寫），並在設定 UI 可選這三種 kind；同人重複 bind 維持 no-op。對齊 design「同人重複 bind 維持 no-op」與 modified spec「Bind identity to person API」。驗證：整合測試覆蓋 gitlab_user 綁定與同人 no-op。

## 3. People settings UI

- [x] 3.1 新增獨立 people AppView（左清單 + 右表單）：建立人員、編輯顯示名、identity 增刪（三種 kind）、參與專案唯讀；不提供刪除人員；保留未歸戶 header 面板。對齊 design「獨立 AppView，不與報告閱讀器左欄合併」與 spec「People settings UI manages persons and identities」。驗證：`npm run build`；手動確認主要流程可操作。

## 4. Docs

- [x] 4.1 更新 `README.md` 與 `docs/idea/schema.md`：記載人員詳情／更名／解除綁定 API，以及更名只搬 `_people/` 的限制。驗證：文件內容審查含端點與限制說明。


