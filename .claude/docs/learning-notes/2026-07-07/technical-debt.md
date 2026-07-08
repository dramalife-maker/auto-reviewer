# 遺留技術債：commit 時工作區混有多任務改動

<tl;dr>
- **何時要想起這則：** commit 前工作區同時有無關 WIP（例如 identity 與 cursor 改動並存）時。
- **不要做：** 一次 stage 全部變更、把無關任務混在同一 commit。
- **要做：** 只 stage 與本次任務相關檔案；必要時 stash 相依 WIP，commit 後還原。
- **意圖：** 只 commit 與本次任務相關的變更，保持 git 歷史可讀。
</tl;dr>

## 使用者為何希望這樣改（意圖）

使用者希望 **只 commit 與本次 cursor 整合任務相關的變更**，避免 person-identity-resolution 等進行中改動混入同一 commit，讓 PR／歷史易於 review。

## 問題描述

commit 時工作區混有 person-identity-resolution 進行中改動與 cursor 改動。

## 解決方法

- 只 stage cursor 相關檔案。
- 必要時 stash 相依 identity 改動；commit 後還原 WIP。

## 避免方法

commit 前用 `git status` / `git diff` 確認 staged 範圍；多任務並行時優先 selective stage 或 stash。
