# 知識缺口：SQLite `datetime('now')` 無時區後綴，前端 Date.parse 會當本地時間

<tl;dr>
- **何時要想起這則：** 前端用 `Date.parse` / `new Date(...)` 計算 SQLite 寫入的 `started_at`、`finished_at`、elapsed 等時間差時。
- **不要做：** 對 `YYYY-MM-DD HH:MM:SS`（或把空白換成 `T` 後）直接 `Date.parse`——無 `Z`／offset 時 ES 視為**本地時間**。
- **要做：** 確認後端來源是 UTC（SQLite `datetime('now')` 即是）後，補 `Z`（或明確 offset）再 parse；elapsed = `Date.now() - parseAsUtc(started_at)`。
- **症狀：** UTC+8 下一按執行 elapsed 從 `480:01` 起跳（剛好 8×60 分）。
- **自問（可選）：** 這個字串有沒有時區？後端存的是 UTC 還是本地？顯示與相減是否同一套假設？
</tl;dr>

## 使用者為何希望這樣改（意圖）

專案列表 running 狀態旁要顯示「這場 run 已跑多久」的即時計時（`MM:SS`），方便判斷是否卡住。

## 問題描述

一按「執行」，`.project-list-elapsed` 立刻顯示約 `480:xx`，而非從 `00:00` 起跳。使用者猜測與時區偏移 480 分（UTC+8）有關——屬實。

## 錯誤原因／學到的知識

| 層 | 行為 |
|----|------|
| SQLite | `datetime('now')` 寫入 **UTC**，字串形如 `2026-07-12 06:03:00`，**無**時區後綴 |
| `Date.parse('...T06:03:00')` | 無 `Z`／`±HH:MM` 時當**本地時間** |
| UTC+8 機器 | 實際現在 ≈ UTC 06:03，但 parse 把「開始」當成本地 06:03 → 少算了 8 小時的「開始點」→ 差額 ≈ **480 分鐘** |

與 schedule UI 的「時區設定」無關；是 JS 日期解析慣例 + SQLite UTC 字串的組合坑。

## 解決方法

`formatRunElapsed`：正規化空白為 `T` 後，若尚無時區後綴則補 `Z`，再 `Date.parse`。

## 避免方法

- 凡從 API／DB 來的「無 zone 時間字串」要做算術，先問：**這是 UTC 還是本地？**
- 相減用 epoch ms；純展示可另轉本地，但**不要**用「無 zone 字串」當本地再跟 `Date.now()` 比。
- 若日後 API 改回傳 RFC3339（含 `Z`），parser 應已帶 zone 則不要重複加 `Z`。

## 相關檔案

- `frontend/src/app.ts`（`formatRunElapsed`）
- `backend/src/runs.rs`（`datetime('now')` 寫入 `runs.started_at` / `run_projects.started_at`）
