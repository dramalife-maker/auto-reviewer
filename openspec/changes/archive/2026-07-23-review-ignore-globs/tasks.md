## 1. 設定儲存層

- [x] 1.1 [P] 新增 migration `backend/migrations/017_review_settings.sql`，交付「Global review ignore list is persisted in the database」：建立單列設定表、插入 id=1 初始列、ignore_globs 預設為空陣列字面值、寫入 schema 版本紀錄。此即設計決策「忽略清單採全域單列設定表而非 per-project 欄位」的落地。驗證：對全新資料庫套用 migration 後查詢該列，斷言回傳空陣列。
- [x] 1.2 為正規化與驗證規則先寫失敗測試，覆蓋「Ignore list entries are normalized and validated on write」的全部案例：去空白、去重保留首次順序、丟棄空白項、冒號開頭被拒、單條超過 200 字元被拒、超過 100 條被拒。驗證：測試存在且因功能尚未實作而失敗。
- [x] 1.3 於 `backend/src/review_settings.rs` 實作清單讀取、全量寫入與正規化驗證函式，並在 `backend/src/lib.rs` 註冊模組。依設計決策「pathspec magic 前綴由後端組裝，拒絕使用者自帶前綴」，儲存原始字串、不在此層加任何前綴。驗證：1.2 的測試全數轉綠。

## 2. 設定 API

- [x] 2.1 於 `backend/src/server.rs` 提供 GET 與 PUT 兩支端點，交付「Review settings API reads and replaces the ignore list」：PUT 依設計決策「設定以全量覆蓋方式更新」採全量取代語意並回傳正規化後結果，違規輸入回應 400 且指出違反的規則、不改動既有值。驗證：新增整合測試，先 PUT 再 GET 斷言內容一致，並斷言違規輸入回應 400 且儲存值未變。

## 3. MR 素材過濾

- [x] 3.1 為素材產生先寫失敗測試，覆蓋「MR change materials exclude ignored files from diff content only」：在同時變更一般原始檔與符合忽略規則之檔案的測試倉庫上，斷言 diff 檔不含被忽略檔案的差異內容、stat 檔仍列出該檔名，且清單為空時 diff 內容與未套用規則時相同。此即設計決策「忽略只作用於 diff 內容，stat 與 log 保留完整」的驗收。驗證：測試存在且失敗。
- [x] 3.2 於 `backend/src/mr_change_materials.rs` 讓素材產生函式接受忽略清單參數，僅對輸出 change.diff 的 diff 指令附加排除用 pathspec，stat 與 log 指令維持原樣，且不附加嚴格 glob magic 以保留跨目錄匹配。驗證：3.1 的測試全數轉綠。
- [x] 3.3 實作「Pathspec failure degrades to an unfiltered diff」：帶 pathspec 的 diff 失敗時記錄警告並改以不帶 pathspec 重跑，重跑仍失敗才回報既有 git 錯誤，對應設計決策「帶 pathspec 的 diff 失敗時降級重跑」。驗證：新增測試，傳入會使 git 失敗的規則，斷言素材仍成功產出且 diff 內容等同不帶規則的結果。
- [x] 3.4 於 `backend/src/worker.rs` 在進入 MR 迴圈前讀取一次清單並沿用於該場 run 的所有 MR，落實設計決策「設定於每場 MR 掃描開始時讀取一次」；測試替身路徑傳入空清單以維持現行輸出。驗證：既有 MR 掃描測試維持通過，並新增斷言確認同一場 run 內只讀取一次設定。

## 4. Manifest 與 agent 約束

- [x] 4.1 於 `backend/src/runs.rs` 為週報與 MR 兩種 manifest 結構加入忽略清單欄位，交付「Run manifests expose the ignore list to agents」：清單非空時序列化該欄位、為空時略過。驗證：擴充既有 manifest 寫入測試，分別斷言非空與空清單兩種序列化結果。
- [x] 4.2 [P] 於 `skills/reviewer-batch/WORKFLOW.md` 與 `skills/scan-mrs-headless/WORKFLOW.md` 加入硬性指示，要求 agent 自行執行的 git 指令一律附帶由 manifest 忽略清單構成的排除 pathspec，對應設計決策「週報與 MR agent 透過 manifest 取得清單並在 WORKFLOW 中約束」。驗證：內容審閱，確認兩份文件的素材收集段落皆含該指示且說明其為軟約束。

## 5. 前端編輯介面

- [x] 5.1 [P] 於 `frontend/src/types.ts` 與 `frontend/src/api.ts` 加入設定型別與讀取／更新兩支呼叫，使頁面能取得並送出忽略清單。驗證：前端型別檢查通過，且 5.3 的測試可對這兩支呼叫做替身。
- [x] 5.2 於 `frontend/src/pages/DashboardPage.tsx` 排程卡片下方新增獨立卡片，交付「Dashboard exposes the ignore list for editing」：每行一條的文字框、獨立儲存按鈕、空清單時顯示常見寫法的 placeholder、儲存成功後提示變更於下一場 run 生效且無須重啟。此即設計決策「前端以獨立卡片與獨立儲存動作呈現」。驗證：由 5.3 的測試覆蓋。
- [x] 5.3 比照 `frontend/src/pages/DashboardPage.catchup.test.tsx` 的風格新增測試：輸入多行內容並觸發儲存後，斷言以原始輸入呼叫更新端點並顯示成功提示；端點回應 400 時斷言顯示錯誤訊息且不顯示成功提示。驗證：測試通過。

## 6. 整體驗證

- [x] 6.1 執行後端測試、前端測試與 lint，確認全數通過且無新增警告。驗證：三項指令皆回傳成功。
- [x] 6.2 手動驗證清單留空時的等價性：以空清單跑一次 MR 掃描，比對產生的三個素材檔與本變更前的輸出一致；再設定一條規則重跑，確認 diff 不含該類檔案而 stat 仍列出。驗證：人工比對結果記錄於變更說明。
