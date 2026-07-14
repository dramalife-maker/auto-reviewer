# output-contract.md — MR review 草稿輸出契約

> 後端 `reviewer-server` 掃描 `draft_dir` 後解析此格式，upsert 進 `mr_reviews`（`status='draft'`）。  
> **frontmatter 鍵名為固定契約**（後端解析）；**body 段落為寫作契約**（收件匣／發佈內容，後端不解析 heading）。  
> **讀者**：MR 作者／GitLab 討論。對人思維模式見 `observation-guidelines.md`，**不要**寫進本草稿。

---

## 檔案位置

```
{draft_dir}/mr-{mr_iid}-round-{review_round}.md
```

- `{draft_dir}` ← manifest `draft_dir`
- 檔名建議含 `mr_iid` 與 `review_round`，但解析以 frontmatter 為準

---

## 輸出格式範例（round 1，填滿）

```markdown
---
mr_iid: 68
mr_title: "feat: TS wallet provider (FunTa)"
review_round: 1
author_identity: gary@co.com
---

## 審查摘要：feat: TS wallet provider (FunTa)（第 1 輪）

### ✅ 做得好的地方
- 掉包恢復設計完整（`packet_id` 冪等 + `05-259` / `checkPacket`），註解清楚
- WebSocket pool 與 `wallet/ts`、`wallet/ts/websocket` 測試覆蓋到位
- 整體分檔與既有 wallet provider 慣例一致

### ❌ 需要修正
| # | 嚴重度 | 問題 | 修法 |
|---|--------|------|------|
| F1 | [高] | `wallet/ts/cache.go`、`api.go` `BetNSettle`（約 L340–346）：註解要求送出前落地 `packet_id`，但 `setCache` 未檢查 `Set().Err()`；Redis 寫入失敗仍送 `GameSpin`，跨請求重試可能換新 UUID，破壞冪等鏈 | `setCache` 改回傳 `error`；**送出 FunTa 前**失敗則直接 return；`defer` 補寫失敗至少 `ErrorLog` |
| F2 | [低] | `ReconnectPool.Close()`（`pool.go` L284–287）留有 graceful shutdown TODO，易被當成待辦 | 若不做：刪 TODO，改一句說明現行行為（in-flight 失敗由上游／`packet_id` 恢復） |
| F3 | [低] | `applyDetail` 傳 `useAccTotalWin` bool，slot 特例散在 caller | 改傳 `gameType`，規則內聚到 method 內 |

### ❓ 建議追問
- Q1 `ValidateRedirectToken` 的 `PlayerType` 目前固定 `Normal`（L625，有 TODO）。FunTa proto 無 `player_type`——是否存在測試／特殊玩家？若無，可否關閉 TODO 並註解說明？
- Q2 FunTa 無取消 API 時，`CancelBet` 改查權威餘額收斂是否已與對方對過邊界案例？

### 💡 整體評估
架構與既有 wallet 慣例一致，掉包恢復與 pending worker 銜接是亮點，測試品質不錯。合併前建議優先處理 **F1 Redis cache 寫入檢查**；其餘為釐清（Q1）或小幅可讀性調整。
```

---

## 輸出格式範例（round 2+，填滿）

```markdown
---
mr_iid: 68
mr_title: "feat: TS wallet provider (FunTa)"
review_round: 2
author_identity: gary@co.com
---

## 審查摘要：feat: TS wallet provider (FunTa)（第 2 輪）

### 📋 上一輪疑慮處理狀態
| # | 上一輪項目 | 狀態 | 備註 |
|---|------------|------|------|
| F1 | `setCache` 送出前檢查 Redis 寫入 | 已解 | `92c9b737`：送出前失敗 abort；`RedisError(nil)==nil` 成功路徑不誤擋 |
| F2 | `Close()` TODO | 已解 | 已改說明註解 |
| F3 | `applyDetail` 改傳 `gameType` | 已解 | 已改 |
| Q1 | `PlayerType` 是否有非 Normal | 已解 | 改依 `internalTesting`／`agent.IsTest` 映射；維護放行邊界正確 |

### ✅ 做得好的地方
- 上輪四項處理紮實，回覆清楚；[高] cache 冪等修正正確

### ❌ 需要修正
| # | 嚴重度 | 問題 | 修法 |
|---|--------|------|------|
| F4 | [高] | 同批夾帶的 `GetPortal` custom portal（`config.go:104-111`）：每次 `return` 臨時、無 `poolCfg` 的 `PortalClient` → 現況不可用；若只補 poolCfg 會每筆新建 pool 且不 `Close` → 連線洩漏 | 依 `agent.PortalHost` 建一次完整 client，map+鎖快取複用 |
| F5 | [低] | `GetPortal` 註解仍寫「不支援 custom portal」與程式矛盾 | 同步修正註解 |

### ❓ 建議追問
- Q2 `internalTesting` 讀取端仍信任 URL query；竄改是否會影響下游過濾？此階段是否接受暫不改、後續改讀取端？

### 💡 整體評估
上輪追蹤項均已關閉。本輪焦點為夾帶的 `GetPortal`（F4）：合併前需補齊 pool 設定與快取複用，並建議補整合測試。Q2 為信任邊界取捨，確認後再動。
```

---

## 骨架（對照用）

```markdown
## 審查摘要：{MR 標題}（第 X 輪）

### 📋 上一輪疑慮處理狀態（僅 round 2+）
| # | 上一輪項目 | 狀態 | 備註 |
|---|------------|------|------|
| F1 | ... | 已解 / 未解 / 部分 / 已過時 | ... |

### ✅ 做得好的地方
- ...

### ❌ 需要修正
| # | 嚴重度 | 問題 | 修法 |
|---|--------|------|------|
| F1 | [高]   | ...  | ...  |

### ❓ 建議追問
- Q1 ...

### 💡 整體評估
（技術／合併判斷 only；不含思維模式；勿手寫 By: AI Agent）
```

---

## Frontmatter（YAML）— 硬契約

| 鍵 | 必填 | 型別 | 說明 |
|----|------|------|------|
| `mr_iid` | 是 | integer | GitLab MR internal id（!number） |
| `mr_title` | 是 | string | MR 標題 |
| `review_round` | 是 | integer | 由 triage 判定（`1`、`2`、…） |
| `author_identity` | 是 | string | MR author email 或 glab username；後端比對 `person_identities` 得 `person_id` |

規則：

- 以 `---` 開頭與結束。
- 缺 `mr_iid` 或 `review_round` 的檔案會被跳過並記 warning。
- **不要**在 frontmatter 寫入 `session_id`（由後端從 agent stdout 擷取）。

---

## Body（Markdown）— 寫作契約

**人眼／API／GitLab note 只看到 body**：後端 list／publish 會剝除 YAML frontmatter；磁碟檔案仍保留 frontmatter 供 ingest。

### 必含段落（順序固定；heading 文字建議照範本）

| 段落 | round 1 | round 2+ | 說明 |
|------|---------|----------|------|
| `## 審查摘要：…（第 X 輪）` | 必填 | 必填 | 標題列 |
| `### 📋 上一輪疑慮處理狀態` | **省略** | 必填 | 對 `prior_published_reviews`／GitLab AI notes 逐條狀態；**沿用上一輪 F／Q 編號** |
| `### ✅ 做得好的地方` | 必填 | 必填 | bullet |
| `### ❌ 需要修正` | 必填 | 必填 | 確定要改；無項可寫「無」 |
| `### ❓ 建議追問` | 建議（round 1 宜充足） | 建議 | 需作者／產品釐清；不確定放這裡，勿硬塞 ❌；**技術選擇**題須先對照 `manifest.notes_dir`（見 WORKFLOW「專案 ADR」），已知 ADR 不得再問 |
| `### 💡 整體評估` | 必填 | 必填 | **只寫技術／合併建議**（不含思維模式） |

### 格式規則

- **❌** = 確定要改；**❓** = 需作者／客戶確認或設計取捨 — **不確定就放 ❓**，不要硬塞進 ❌。
- **每一項都要有編號**：❌ 用 `F1`、`F2`…；❓ 用 `Q1`、`Q2`…，方便追蹤。
- round 2+「上一輪疑慮處理狀態」**沿用上一輪編號**（`F1 → 已解`），不要重編。
- 避免多層巢狀 bullet。
- `[高]` 每份 review **建議 ≤ 3 項**；且須通過寫入前事實查核（見 `WORKFLOW.md` §2.3）。
- **不要**附評分表（功能完成度／文件對齊等 /10），除非管理者在追問／agent-turn 中明確要求。
- round 1：❓ 為主、少給 ❌ 修法；integration／語意不明時尤其如此。
- round 2+：先上一輪表，再本輪 ❌／❓。

### 需要修正 — 表格與嚴重度

- 嚴重度：`[高]`／`[中]`／`[低]`。
- **問題**欄：含位置（檔案／函式／行號）＋為何是問題。
- **修法**欄：可操作建議（路徑、符號、範例）；勿只寫「請修正」。
- 按優先順序排列（高 → 低）。

### 上一輪疑慮（round 2+）

- 狀態用：`已解`／`未解`／`部分`／`已過時`。
- 備註須可驗證（commit／檔案／討論）；勿只寫「作者有改」。
- 本輪聚焦新風險、未關閉項、討論新議題——避免整份重寫 round 1。

### 整體評估 — 禁止事項

- **禁止**工程師思維模式、人格、1on1 敘事（只寫觀察片段）。
- 寫：架構是否可接受、合併前必修項、測試／風險收束。

### `By: AI Agent`

- **不要**在草稿末自行加 `By: AI Agent`；發佈時後端會附加。

---

## 觀察片段（另檔 — 非本契約）

觀察寫作規範見 `observation-guidelines.md`。同一場次落兩處：

1. `{pending_dir}/mr-{mr_iid}-round-{review_round}.md`
2. `{person_month_md_path}`（專案層月檔追加）

本檔**只約束草稿**。不要把觀察的對人／思維模式敘事寫進草稿 body。

---

## 後端解析對照

| 產出 | 寫入 |
|------|------|
| frontmatter `mr_iid` / `review_round` | `mr_reviews` upsert key `(project_id, mr_iid, review_round)` |
| `author_identity` | 查 `person_identities` → `person_id`（比對不到則 `NULL`） |
| 檔案路徑 | `mr_reviews.draft_md_path` |
| body（無 frontmatter） | 收件匣顯示；發佈時可編輯後 POST 至 GitLab（後端附加 `By: AI Agent`） |
