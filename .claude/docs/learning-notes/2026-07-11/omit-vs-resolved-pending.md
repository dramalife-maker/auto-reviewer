# 需求誤解：週報「省略待確認」不可推斷為已解決

<tl;dr>
- **何時要想起這則：** 改週報 workflow／`summary.md` 契約、pending 閉環、ingest 自動 resolve，或討論「同週重跑不要重複問題」時。
- **不要做：** 以「沒再出現在 `## 待確認`」自動把 open pending 標成 resolved；假設 headless workflow 能寫 SQLite；把「本週不提」與「已釐清」合成同一種省略。
- **要做：** 延續議題用 `manifest.open_pending` **原文**寫入 `## 待確認`；已釐清用固定 heading `## 已釐清`（同樣原文）由 ingest resolve；兩區都不寫 = 仍 open。精確字串匹配；SQLite 查 `project_id` 維度的 open list 需 leading-`project_id` 索引。
- **意圖：** 同週重跑不因措辭不同複製 open 列，又能在真的釐清時自動閉環，且不誤關「這週先不提」的項。
- **自問（可選）：** 這條路徑是 agent 寫檔還是後端寫 DB？省略與解決是否需要兩個可觀測訊號？
</tl;dr>

## 使用者為何希望這樣改（意圖）

同週重跑週報時，相同待確認不應因 LLM 改寫而變成新 open 列；若 agent 發現議題已釐清，也應能自動 resolve，不必每次靠人勾 checkbox。

## 問題描述

初版去重只靠 DB 精確字串；重跑常改寫問句。若進一步「沒寫進待確認就 resolve」，會把「本週省略」誤關。workflow 契約又禁止寫 SQLite，閉環只能由後端 ingest 完成。

## 錯誤原因

把「產物裡沒出現」當成「狀態機上已結束」。省略是**呈現選擇**，解決是**狀態變更**；兩者需要不同訊號。另：既有 pending 索引多以 `person_id` 為最左欄，`WHERE project_id = ? AND status = 'open'`（manifest 載入）用不上，需另建 `(project_id, …) WHERE status='open'`。

## 解決方法

1. Manifest 注入 `open_pending`；workflow 延續議題必須原文。
2. `summary.md` 第四區 `## 已釐清`；ingest 精確匹配 open 列後走共用 `resolve_pending_item`（notes 失敗 warn 後繼續）。
3. Migration `012_pending_open_by_project.sql` 補 project 維度 open 索引。

## 避免方法

- Agent 產檔＋後端入庫的系統：**狀態變更必須有顯式、可解析的欄位／heading**，不要靠「缺席」。
- 設計「可省略」時先問：省略後 DB／UI 應如何？若仍 open，就**禁止**用缺席觸發 resolve。
- 新 SQL 以非索引最左欄過濾時，對一下 `EXPLAIN`／既有 migration 索引前綴。

## 相關檔案

- `skills/reviewer-batch/WORKFLOW.md`
- `skills/reviewer-batch/output-contract.md`
- `backend/src/runs.rs`
- `backend/src/summary.rs`
- `backend/migrations/012_pending_open_by_project.sql`
- `openspec/specs/reviewer-execution/spec.md`
- `openspec/specs/pending-closure/spec.md`
