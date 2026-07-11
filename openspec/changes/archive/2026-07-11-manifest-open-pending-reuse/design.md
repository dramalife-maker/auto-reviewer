## Context

週報 ingest 已對 open `pending_items` 做 `(person_id, project_id, question)` 精確去重，但 agent 重跑時常改寫問句，導致語意重複列。UI 以 DB open 列為準；workflow 目前只讀 manifest，看不到既有 open 問題。

## Goals / Non-Goals

**Goals:**

- 寫週報 manifest 前載入該專案所有 `status='open'` 的 pending
- workflow 延續議題必須沿用 manifest 原文；不再相關者可從本次 `## 待確認` 省略
- 省略不自動 resolve；管理者仍在 UI 閉環

**Non-Goals:**

- 語意相似度 / embedding / LLM judge 合併（想法 3）
- 穩定 slug / topic key（想法 2）
- 省略時自動把 DB 標為 resolved
- 沿用舊題時強制跳過 `_notes.md` append（使用者選「可省略」未要求檔案層去重）
- 變更 ingest 去重規則（精確字串去重維持）

## Decisions

### Manifest 注入 open pending

- **選擇**：在 `RunManifest` 新增 `open_pending` 陣列；每元素含 `id`, `person_id`, `display_name`, `question`。查詢條件：`pending_items.project_id = 本專案` 且 `status = 'open'`，JOIN `people` 取 `display_name`。空陣列時以 `skip_serializing_if` 省略欄位（與 `published_pending_snippets` 一致）亦可，但測試應對空／非空皆可解析；實作採「總是序列化陣列（可為空）」較利於 workflow 判斷欄位存在。
- **替代**：只注入本次 `authors` 的人 → 否決；非本週活躍但仍 open 的人應保留在清單，供後續週次沿用。
- **替代**：按人嵌在 `authors[]` 內 → 否決；open 與本週活躍集合不完全重疊，扁平清單較單純。

### Workflow 沿用或省略

- **選擇**：硬性規則——若要把某條既有 open 議題寫進本次 `## 待確認`，bullet 文字 MUST 等於 manifest 對應 `question`；禁止同義改寫。若本週不再相關，可省略該條；DB 列保持 open。
- **替代**：open 必須每週回聲 → 否決（使用者選擇可省略）。

### 測試策略

- **選擇**：擴充或新增整合測試：seed open `pending_items` 後呼叫 `write_weekly_manifest`，斷言 `open_pending` 含預期 `id`／`question`／`display_name`。
- Workflow / output-contract 為文件契約；以文件審查驗證規則文字存在。

## Implementation Contract

- **Behavior**: 每次週報專案執行前寫出的 `manifest.json` 含該專案目前所有 open pending。Agent 延續議題時必須原句寫入 `## 待確認`；可省略不再相關項且不 resolve。
- **Interface / data shape**:
  - `open_pending`: 陣列；元素 `{ "id": number, "person_id": number, "display_name": string, "question": string }`
  - 查詢範圍：單一 `project_id`、`status='open'`
- **Failure modes**: DB 查詢失敗則整次寫 manifest 失敗（與既有 authors／snippets 載入一致）；無 open 列時寫空陣列。
- **Acceptance criteria**:
  - 整合測試：有／無 open pending 時 manifest JSON 形狀正確
  - `WORKFLOW.md` 與 `output-contract.md` 記載沿用原文與可省略規則
- **Scope boundaries**:
  - In: `write_weekly_manifest`、workflow／contract 文件、schema 文件、manifest 測試
  - Out: ingest 邏輯、前端、語意合併、自動 resolve、`_notes.md` 去重

## Risks / Trade-offs

- [Agent 仍改寫] → Mitigation：依賴硬性 workflow 文案；後續才考慮想法 3 後備
- [省略後 UI 仍顯示舊題] → 預期行為；管理者在 checkbox 閉環
- [Manifest 變大] → open 數量通常很少（每人每專案個位數）
