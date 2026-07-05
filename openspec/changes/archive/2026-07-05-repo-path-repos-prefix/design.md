## Context

MVP 已實作 `projects.yaml` 載入，但 `repo_path` 未與 `DATA_ROOT_DIR/repos/` 慣例綁定。產品文件（`docs/idea/spec.md` §9.0）約定 clone 放在 `$DATA_ROOT_DIR/repos/<slug>/`，設定檔應支援簡短 slug 以降低錯誤。

## Goals / Non-Goals

**Goals:**

- 使用者可寫 `repo_path: test/projectA`，實際目錄為 `$DATA_ROOT_DIR/repos/test/projectA`。
- DB 存**解析後**路徑；worker / manifest 沿用既有 `projects.repo_path` 欄位，無 API 變更。
- 單元測試覆蓋 slug、絕對路徑、明確相對路徑三種情況。

**Non-Goals:**

- 不變更 `name` 與 `repo_path` 的自動對應（仍為獨立欄位）。
- 不實作 `git_remote_url` 自動 clone。
- 不遷移既有 DB 列（啟動時 upsert 會以新解析結果覆寫）。

## Decisions

### repo_path 解析規則

在 `load_from_yaml` 內、git 偵測與 DB upsert **之前**解析：

| 輸入類型 | 條件 | 解析結果 |
|----------|------|----------|
| 絕對路徑 | `Path::is_absolute()` | 原樣使用 |
| 明確相對路徑 | 第一個 path component 為 `.` 或 `..` | 原樣使用（相對 cwd，相容舊範例） |
| Repo slug | 其餘相對路徑 | `data_dir.join("repos").join(raw)` |

範例（`DATA_ROOT_DIR=./data/reviewer`）：

| YAML `repo_path` | 解析後 |
|------------------|--------|
| `game-backend` | `./data/reviewer/repos/game-backend` |
| `test/projectA` | `./data/reviewer/repos/test/projectA` |
| `./data/reviewer/repos/legacy` | `./data/reviewer/repos/legacy`（不雙重 prefix） |
| `/var/reviewer/repos/foo` | `/var/reviewer/repos/foo` |

實作位置：`backend/src/projects.rs` 新增 `resolve_repo_path(data_dir, raw) -> PathBuf`，`load_from_yaml` 傳入 `AppConfig::data_dir()`。

### 儲存格式

- DB 存 `PathBuf::display().to_string()`（與現有測試一致）。
- 不在 YAML 保留原始 slug 與解析路徑雙欄（YAGNI）。

## Risks / Trade-offs

- **[Risk] 舊設定使用 cwd 相對但無 `./` 前綴**（如 `repos/foo`）→ 會被視為 slug，解析到 `$DATA_ROOT_DIR/repos/repos/foo`。緩解：文件說明；此寫法本來就不符合慣例，影響面小。
- **[Risk] Windows 絕對路徑** → 使用 `Path::is_absolute()`，無需特殊處理。

## Migration Plan

1. 部署新版後端。
2. 可選：將 `projects.yaml` 改為 slug 寫法；舊的 `./data/...` 寫法無需修改。
3. 重啟後 upsert 自動更新 DB 中的 `repo_path`。
