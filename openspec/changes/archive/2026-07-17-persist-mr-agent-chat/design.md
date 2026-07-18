## Context

MR 收件匣 Agent Chat 已透過 `POST /api/mr-reviews/:id/agent-turn` 接續 provider session，但對話只存在前端 React state；重整後會清空。`mr_reviews` 已有 `agent_session_id` / `reviewer_agent`，缺應用層 transcript。

實務上 agent CLI 可在 worktree 內直接改 `draft_md_path` 對應檔案；這是預期能力（規格舊文「handler 不修改 draft」僅指後端程式不主動覆寫，不禁止 agent 改檔）。前端編輯器若不同步，會在 dirty 狀態下與磁碟新版衝突。

## Goals / Non-Goals

**Goals:**

- 成功的 agent-turn 將 user + assistant 訊息持久化到 SQLite
- 列表 API 帶回訊息，前端重整後還原
- 已發布／已忽略可唯讀回顧歷史；僅 draft 可繼續追問
- agent-turn 成功後重讀草稿並回傳；UI 標記新版本並處理與本地編輯的衝突
- 儲存草稿時以基準 hash 做樂觀鎖定，避免靜默覆寫 agent 改過的檔案

**Non-Goals:**

- 不把初次產出草稿的 agent stdout 寫入 chat（草稿本體仍在檔案）
- 不自動三方 merge，亦不做 unified line-diff（衝突時以唯讀預覽新版對照編輯器）
- 不提供訊息編輯／刪除／分頁 API
- 不遷移或還原已遺失的記憶體對話
- 不強制 agent 一定要改草稿（沒改檔就不顯示新版本）

## Decisions

### 獨立訊息表而非 JSON 欄位

新增 `mr_review_chat_messages`（`mr_review_id` FK `ON DELETE CASCADE`、`role`、`content`、`created_at`），以 `id` 遞增作為對話順序。否決 JSON blob：不利測試與查詢，且與既有關聯表風格不一致。

### 僅成功回合落庫

`agent_turn` 在 CLI 成功回覆後插入 user 與 assistant 兩列；失敗不寫入。前端可暫時顯示未成功的 user 气泡，重整後只留已提交成功的回合。

### 列表嵌入 `chat_messages`

擴充 `GET /api/mr-reviews` 每筆的 `chat_messages`（依 `id` ASC）。不另開 detail endpoint。

### 生命週期與唯讀規則

訊息與 `mr_reviews` 同壽命；publish／ignore 不刪 transcript。UI：`draft` 可輸入（需 session）；`published`／`ignored` 有訊息才顯示唯讀 chat。

### Agent 改草稿為預期行為

後端 `agent_turn` 本身不寫入草稿檔，但成功後 MUST 重讀 `draft_md_path`（strip frontmatter 後）並放入回應的 `draft_body`。內容相對請求前是否變化由前端用基準字串比對。

### 基準草稿與衝突 UX

前端為每個選中 draft 記住 `baselineDraftBody`（上次從伺服器認定的內容）。agent-turn 成功且回傳 `draft_body` 後：

- 編輯器未 dirty 且內容不同 → 套用新內容、更新 baseline、顯示可關閉的「草稿有新版本」標記
- 編輯器 dirty 且內容不同於回傳稿 → 顯示衝突橫幅，三動作：
  - **預覽新版本**：唯讀顯示伺服器／agent 回傳的 `draft_body`（沿用既有 Markdown Preview），不改動編輯器文字
  - **載入新版本**：放棄本地編輯，套用新稿並更新 baseline
  - **保留我的編輯**：關閉衝突提示，保留 dirty 文字（之後儲存可能覆寫磁碟）
- 不做行內 merge、hunk 接受／拒絕、或 unified line-diff（預覽全文對照編輯器即可）；日後若不足再加 readonly diff
- 選取 review 時若 `draft_body` 因外部更新而變，不得在 dirty 時強制重置編輯器（修正現有 effect 行為）

### PATCH 樂觀鎖定與 `draft_hash`

後端對 strip 後的 `draft_body` 計算穩定 SHA-256 hex 為 `draft_hash`。`GET /api/mr-reviews` 與 `agent-turn` 回應皆帶上目前 `draft_hash`，前端以伺服器值當基準，不自行雜湊。

`PATCH /api/mr-reviews/:id` 接受可選 `base_hash`。若提供且與目前檔案 body 的 hash 不符 → HTTP 409，JSON 含目前 `draft_body` 與 `draft_hash`。未帶 `base_hash` 時維持直接覆寫（相容舊測試），但收件匣 UI MUST 帶上 hash。

## Implementation Contract

**行為：**

- 成功 `agent-turn` 後 DB 多兩列（user 再 assistant）；回應含 `reply`、`agent_session_id`、`draft_body`、`draft_hash`
- 重新載入收件匣後可見先前成功對話
- 非 draft 的 agent-turn 仍 409
- dirty 衝突時可預覽新版後再選載入或保留；未 dirty 自動跟新版並標記
- 帶錯誤 `base_hash` 的 PATCH 不覆寫檔案

**介面／資料形狀：**

- Migration `013_mr_review_chat_messages.sql`：
  - `id INTEGER PRIMARY KEY AUTOINCREMENT`
  - `mr_review_id INTEGER NOT NULL REFERENCES mr_reviews(id) ON DELETE CASCADE`
  - `role TEXT NOT NULL`（`user` | `assistant`）
  - `content TEXT NOT NULL`
  - `created_at TEXT NOT NULL DEFAULT (datetime('now'))`
  - index on `mr_review_id`
- `GET /api/mr-reviews` item 新增 `chat_messages`（可空）與 `draft_hash`
- `POST .../agent-turn` → `200 { reply, agent_session_id, draft_body, draft_hash }`
- `PATCH .../:id` → `{ draft_body, base_hash? }`；hash 衝突 → `409` + `{ draft_body, draft_hash }`

**失敗模式：**

- agent 執行失敗：不插入訊息；不因「未改檔」而失敗
- 插入訊息失敗：整次 agent-turn 失敗（5xx）
- PATCH hash 不符：409，磁碟不變

**Acceptance：**

- 後端：成功 turn 後 list 含兩則訊息；失敗 turn 訊息數不變；turn 回應含重讀後 `draft_body`；PATCH 錯 hash → 409
- 前端：hydrate chat；published 唯讀；未 dirty 自動套用新稿並見標記；dirty 衝突可預覽新版且見載入／保留，預覽不改 editor

**範圍：**

- In：schema、chat 持久化、agent-turn 重讀草稿、PATCH hash、MrInboxPage hydrate／唯讀／新版本／衝突預覽
- Out：行內 merge、unified line-diff、串流、跨 review 搜尋 chat、強制 agent 改檔

## Risks / Trade-offs

- [列表 payload 變大] → 收件匣筆數與輪數通常有限；必要時再 lazy load
- [失敗回合前端短暫顯示 user 訊息] → 重整後消失；toast 即可
- [未帶 base_hash 的舊客戶端仍可覆寫] → 收件匣必帶；測試覆蓋有／無 hash 兩路徑
- [保留編輯後再存會蓋掉 agent 改動] → 衝突 UI 文案明示；不提供自動 merge
