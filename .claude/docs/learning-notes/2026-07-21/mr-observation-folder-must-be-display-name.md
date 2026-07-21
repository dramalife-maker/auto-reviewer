# 架構缺陷：MR observation 資料夾絕不可用 gitlab username

<tl;dr>
- **何時要想起這則：** 寫 MR poll / observation 路徑、`pending_dir`、`person_month_md_path`、或任何 `reports/{project}/{person}/` 命名邏輯時。
- **不要做：** 用 triage 的 `author_identity`（常是 GitLab username）當資料夾名；解不到 person 就 fallback 成 raw identity。
- **要做：** 資料夾名**只**能是 `people.display_name`；解不到就 **skip 該 MR**（與 unmatched commit author gate 同級），絕不建 orphan username 目錄。
- **意圖：** 專案層報告／`_pending`／閱讀器都以 `display_name` 為鍵；username 資料夾會讓 UI 與週報折入找不到檔案。
- **自問：** 這條路徑若只綁了 `git_email`、沒綁 `gitlab_user`，還會不會建出正確目錄？解失敗時會不會偷偷用 username？
</tl;dr>

## 使用者為何希望這樣改（意圖）

報告目錄、閱讀器 pending 掃描、週報 fold-in 都以 `people.display_name` 為唯一資料夾鍵。用 GitLab username 建目錄會造成幽靈資料夾、UI 看不到觀察片段，且此錯誤已重複發生。

## 問題描述

MR review 執行時在 `reports/{project}/` 下建立以 **gitlab username** 命名的人員資料夾，而非 DB 的 `people.display_name`。

## 錯誤原因／學到的知識

- Triage `author_identity` 優先 email，沒有公開 email 時就用 **username**（常見）。
- Worker gate 只驗證 commit 的 **`git_email` 已綁定**。
- 資料夾解析若只查 `author_identity` → `gitlab_user`／`glab_user`，在「只綁 email」時失敗。
- **致命反模式：** 失敗後 `Ok(trimmed_author_identity)` —— 等於用 username 建目錄；修了「優先 display_name」但保留這條 fallback，錯誤仍會再發。
- Identity（email / username）≠ 資料夾鍵；資料夾鍵永遠是 `display_name`。

## 解決方法

1. `resolve_observation_person_folder`：先 `author_identity`，再唯一 commit-author `display_name`；否則回傳 `None`。
2. Worker 收到 `None` → **skip MR**（warn），禁止 fallback 到 `author_identity`。
3. 單元測試鎖住：unbound username、多人 commit author 歧義 → 必須 `None`。

## 避免方法

- 新增 `reports/{project}/{x}/` 路徑時，`x` 必須來自 `people.display_name` 查詢結果。
- 禁止 `create_dir_all` 使用未驗證的 `author_identity`／`%an`／username。
- 解不到 person 時 skip 或進 unmatched 流程，不要「先建目錄再說」。
