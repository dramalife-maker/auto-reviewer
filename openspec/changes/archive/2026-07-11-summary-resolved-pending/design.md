## Context

`manifest-open-pending-reuse` 已注入 `open_pending` 並規定延續議題原文／可省略。省略刻意不 resolve。管理者仍需手動閉環已釐清項。workflow 禁止寫 SQLite，故閉環必須由後端 ingest 完成。

## Goals / Non-Goals

**Goals:**

- `summary.md` 以 `## 已釐清` 顯式列出本週確認已解決的既有 open 問句（原文）
- ingest 後對 `(person_id, project_id, question)` 命中且 `status='open'` 的列執行 resolve（`resolved_date` 用 schedule 時區月份；可選 note；同步 `_notes.md`）
- 僅從 `## 待確認` 省略、未列於 `## 已釐清` 的 open 項保持 open

**Non-Goals:**

- 語意相似度自動合併／自動判斷已解決
- 省略即 resolve
- 變更手動 PATCH 閉環 API 契約
- frontmatter 陣列替代 heading

## Decisions

### 已釐清訊號用固定 heading

- **選擇**：第四個固定 level-2 heading `## 已釐清`；bullet `- ` 文字必須與 open 問句完全相同。
- **替代**：frontmatter `resolved_pending` → 否決；與既有三區 bullet 風格不一致。
- **替代**：省略即 resolve → 否決；無法區分「這週不提」與「已釐清」。

### Ingest 呼叫共用 resolve 路徑

- **選擇**：抽取／重用 `pending_items` 的 resolve 邏輯（DB update + notes sync）。ingest 對每個 `## 已釐清` bullet 查 open 列；命中則 resolve；未命中（無 open 同文）則忽略並記 log，不失敗整次 ingest。
- **resolution_note**：自動閉環預設 null（或固定短註如 `auto-resolved from weekly summary`）；採 null 以保持簡單，與手動無 note 路徑一致。
- **notes 同步失敗**：與 API 一致——DB 已 resolved 時 notes 失敗應記錄 warn；ingest 路徑選擇 **warn 後繼續**（不中止同專案其他 summary），因批次不可因單一人 notes IO 失敗整批回滾。設計上與 HTTP 502 不同：批次 best-effort notes。

### Workflow 規則更新

- 若判斷 `open_pending` 某條已釐清 → 原文寫入 `## 已釐清`，且不得再出現在 `## 待確認`
- 若仍 open 且相關 → 原文寫入 `## 待確認`
- 若仍 open 但不提 → 兩區都不寫該句

## Implementation Contract

- **Behavior**: 週報 ingest 讀到 `## 已釐清` 中與該人該專案 open pending 完全相同的問句時，將該列標為 resolved 並嘗試同步 `_notes.md`。
- **Interface**: `summary.md` 四個固定 heading：`## 本週重點`、`## 成長面向`、`## 待確認`、`## 已釐清`（後者可空）。`ParsedSummary` 含 `resolved_questions: Vec<String>`。
- **Failure modes**: 未知 person 仍跳過整份 summary；已釐清問句無匹配 open 列 → skip + warn；notes sync 失敗 → warn、DB 維持 resolved、ingest 繼續。
- **Acceptance criteria**: 整合測試：seed open 項 → ingest 含 `## 已釐清` 同文 → 列變 resolved 且（可測則）notes 含 resolved 行；僅省略待確認不含已釐清 → 仍 open。
- **Scope**: summary 解析／ingest、pending resolve 重用、workflow／contract／schema 文件、測試。Out: 前端、語意比對。

## Risks / Trade-offs

- [Agent 漏寫已釐清] → 項保持 open（可接受；管理者仍可手動關）
- [Agent 誤標已釐清] → 誤關；依賴原文精確匹配降低誤傷範圍
- [Notes sync 批次 best-effort 與 API 502 不一致] → 文件註明；避免單檔 IO 拖垮整批
