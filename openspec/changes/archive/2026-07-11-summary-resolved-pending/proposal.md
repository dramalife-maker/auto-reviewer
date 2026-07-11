## Why

同週重跑週報時，agent 可能發現既有 open 待確認已在程式／討論中釐清。目前只能省略不寫，DB 仍 open，管理者需手動勾選。需要顯式「已釐清」訊號讓 ingest 自動閉環。

## What Changes

- `summary.md` 新增固定 heading `## 已釐清`；bullet 必須等於該人／專案 `open_pending`（或 DB open）原文
- 週報 ingest 解析 `## 已釐清`：對命中的 open `pending_items` 執行與手動閉環相同的 resolve（含 `_notes.md` 同步）
- 更新 `reviewer-batch` workflow／output-contract：已釐清寫入該區；僅省略 `## 待確認` 仍保持 open
- 補整合測試

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `reviewer-execution`: summary 契約與 ingest 支援 `## 已釐清` 自動 resolve
- `pending-closure`: 週報 ingest 路徑可將匹配的 open 項 resolve（語意與 PATCH 閉環一致，含 notes 同步）

## Impact

- Affected specs: `reviewer-execution`, `pending-closure`
- Affected code:
  - Modified: `backend/src/summary.rs`, `backend/src/pending_items.rs`, `skills/reviewer-batch/WORKFLOW.md`, `skills/reviewer-batch/output-contract.md`, `docs/idea/schema.md`
  - Modified tests: `backend/tests/runs_execution.rs` 或 `backend/tests/pending_items.rs`／`identity.rs`
