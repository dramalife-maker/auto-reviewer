# 需求誤解：sessionStorage dismiss 與「reload 再現」不可同寫

<tl;dr>
- **何時要想起這則：** 寫 banner／toast dismiss、session 級 UI 狀態，或 spec 提到 `sessionStorage` / `localStorage` / 記憶體 dismiss 時。
- **不要做：** 同一條 requirement 寫「存 `sessionStorage`」又寫「之後 reload MUST 再顯示」；未對照實作就定 acceptance。
- **要做：** 先定存活範圍（同 tab session／關 tab 後／永久），再選儲存體；acceptance 只描述該儲存體的真實行為。本專案 catch-up：同 tab F5 仍藏；新 tab 或新 `due_at` 再現。
- **意圖：** 錯過週報橫幅可暫時關掉吵鬧，但不應寫進 DB；關閉分頁後若仍 missed 應再提醒。
- **自問（可選）：** dismiss 之後「重新整理」與「新開分頁」分別應如何？這句 acceptance 用選定的儲存體做得到嗎？
</tl;dr>

## 使用者為何希望這樣改（意圖）

漏跑週報橫幅要能「稍後再說」且不寫資料庫；同一瀏覽階段內不要一直擋視線，但換分頁或新漏跑窗口仍應提醒。

## 問題描述

scheduling spec 同時要求 dismiss 用 `sessionStorage`，以及「之後 reload MUST 再顯示橫幅」。實作依 design 使用 `sessionStorage` keyed by `due_at`，同 tab F5 仍隱藏——與後句矛盾。pre-review 才發現，阻礙簽核。

## 錯誤原因

把「session-only、不進 DB」口語化成「下次進來還會看到」，未區分：

| 行為 | `sessionStorage` | 僅記憶體 | `localStorage` |
|------|------------------|----------|----------------|
| 同 tab F5 | 仍 dismiss | 再現 | 仍 dismiss |
| 新 tab | 再現 | 再現 | 仍 dismiss |

## 解決方法

修正 main spec：明確「同 tab reload 保持隱藏；關 tab／新 tab／新 `due_at` 再現」，並補對應 scenario；實作不變。

## 避免方法

- 寫 dismiss 時先填表：F5、新 tab、關再開、換 key——各要不要再現。
- 選定儲存體後，**禁止**再寫該儲存體做不到的 MUST。
- Spec／design／實作三角：改其一就掃另外兩個的 dismiss 句子。

## 相關檔案

- `openspec/specs/scheduling/spec.md`
- `frontend/src/app.ts`（`CATCHUP_DISMISS_KEY` / `sessionStorage`）
