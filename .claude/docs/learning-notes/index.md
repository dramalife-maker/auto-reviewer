# Learning Notes Index

快速查詢所有錯誤筆記。每次新增筆記後，同步更新此索引。

## 粗心疏忽 (Typo)

| 日期 | 檔案 | 摘要 |
|------|------|------|
| <!-- YYYY-MM-DD --> | <!-- path --> | <!-- 一句話描述 --> |

## 知識缺口 (Knowledge Gap)

| 日期 | 檔案 | 摘要 |
|------|------|------|
| 2026-07-07 | 2026-07-07/knowledge-gap.md | 整合 Cursor Agent CLI：subprocess、Windows 換行、本機 login token 繼承 |
| 2026-07-07 | 2026-07-07/reviewer-spawn-hang.md | Windows `.cmd` shim 內部 `powershell -File %*` 二次解析打散大 prompt，需改直呼 `.ps1`；含診斷法 |

## 架構缺陷 (Architecture Defect)

| 日期 | 檔案 | 摘要 |
|------|------|------|
| 2026-07-07 | 2026-07-07/architecture-defect.md | REVIEWER_* 環境變數不應散落在 executor，應集中至 config.rs 的 AppConfig |
| 2026-07-07 | 2026-07-07/reviewer-spawn-hang.md | 背景 job 執行失敗用 `?` 直接拋出未 finalize，導致 run 永久卡 running 且錯誤被靜默吞掉 |

## 需求誤解 (Requirement Misunderstanding)

| 日期 | 檔案 | 摘要 |
|------|------|------|
| 2026-07-08 | 2026-07-08/reviewer-model-env.md | model 用通用 `REVIEWER_MODEL` 而非 `REVIEWER_CURSOR_MODEL`；未設定則不傳 CLI `--model` |

## 競態條件 (Race Condition)

| 日期 | 檔案 | 摘要 |
|------|------|------|
| <!-- YYYY-MM-DD --> | <!-- path --> | <!-- 一句話描述 --> |

## 遺留技術債 (Technical Debt)

| 日期 | 檔案 | 摘要 |
|------|------|------|
| 2026-07-07 | 2026-07-07/technical-debt.md | 多任務 WIP 並存時 selective stage 或 stash，避免無關改動混入 commit |
