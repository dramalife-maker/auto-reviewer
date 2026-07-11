## Why

同週重跑週報時，agent 常把仍 open 的待確認改寫成新措辭，導致 DB 字串去重失效、UI 出現語意重複的問題。需要在產報前把既有 open 待確認注入 manifest，強制延續議題沿用原文。

## What Changes

- 週報 `manifest.json` 新增該專案目前 `status='open'` 的 pending 清單（含 `id`、`person_id`、`display_name`、`question`）
- `reviewer-batch` workflow 規定：延續中的議題必須一字不差沿用 manifest 原文；不再相關的 open 項可從本次 `## 待確認` 省略（不自動 resolve）
- 更新 `output-contract.md` / schema 文件中的 manifest 欄位說明
- 補上寫入 manifest 的整合測試

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `reviewer-execution`: 週報 manifest 必須帶入專案 open pending；workflow 必須沿用原文或省略，禁止改寫既有 open 問句

## Impact

- Affected specs: `reviewer-execution`
- Affected code:
  - Modified: `backend/src/runs.rs`, `skills/reviewer-batch/WORKFLOW.md`, `skills/reviewer-batch/output-contract.md`, `docs/idea/schema.md`
  - Modified tests: `backend/tests/identity.rs` 或新增週報 manifest 測試
