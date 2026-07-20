# Learning Notes Index

快速查詢所有錯誤筆記。每次新增筆記後，同步更新此索引。

## 粗心疏忽 (Typo)

| 日期 | 檔案 | 摘要 |
|------|------|------|
| <!-- YYYY-MM-DD --> | <!-- path --> | <!-- 一句話描述 --> |

## 知識缺口 (Knowledge Gap)

| 日期 | 檔案 | 摘要 |
|------|------|------|
| 2026-07-20 | 2026-07-20/fixed-position-h-full-height-trap.md | `position: fixed` 元件的 `h-full` 仍需明確高度祖先鏈；祖先只有 `flex flex-col` 無 height 會塌陷成內容高度 |
| 2026-07-18 | 2026-07-18/tailwind-classname-override.md | Tailwind 衝突看 stylesheet 順序；atom 內 `rounded-md` 不會被呼叫端 `rounded-full` 保證蓋過，需 `!`／merge／改 atom |
| 2026-07-13 | 2026-07-13/mr-agent-stdout-pipe-deadlock.md | MR agent `piped` stdout 等 wait 才讀 → 寫滿 ~64KiB pipe 死鎖，假 timeout；手動 CLI 卻幾分鐘就結束 |
| 2026-07-12 | 2026-07-12/sqlite-utc-date-parse.md | SQLite `datetime('now')` 無 zone；前端 Date.parse 當本地 → UTC+8 下 elapsed 從 480 分起跳 |
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
| 2026-07-11 | 2026-07-11/omit-vs-resolved-pending.md | 週報省略待確認 ≠ resolve；已釐清需 `## 已釐清` 顯式訊號 + ingest；含 project 維度 open 索引注意 |
| 2026-07-11 | 2026-07-11/sessionstorage-dismiss-spec.md | sessionStorage dismiss 與「reload MUST 再現」互斥；acceptance 須對齊儲存體真實行為 |

## 競態條件 (Race Condition)

| 日期 | 檔案 | 摘要 |
|------|------|------|
| 2026-07-18 | 2026-07-18/trends-requested-for-effect-race.md | `useEffect` 請求前 `set` 又把該 state 當 deps → cleanup cancel → loading 永遠不關；改 settle 後再標記或用 ref |

## 遺留技術債 (Technical Debt)

| 日期 | 檔案 | 摘要 |
|------|------|------|
| 2026-07-07 | 2026-07-07/technical-debt.md | 多任務 WIP 並存時 selective stage 或 stash，避免無關改動混入 commit |
