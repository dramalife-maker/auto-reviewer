## 1. repo_path 解析（project-config）

- [x] 1.1 【Requirement: Projects load from YAML at startup】在 `backend/src/projects.rs` 實作 `resolve_repo_path(data_dir, raw)`，依 design 三類規則回傳 `PathBuf` — 驗證：`cargo test resolve_repo_path_slug` / `resolve_repo_path_absolute` / `resolve_repo_path_explicit_relative`
- [x] 1.2 【Requirement: Projects load from YAML at startup】`load_from_yaml` 改傳 `data_dir`，解析後以 `display().to_string()` 寫入 DB，再 git 偵測與 upsert；更新 `backend/src/lib.rs` 呼叫端 — 驗證：`cargo test repo_slug_loads_resolved_path`
- [x] 1.3 更新 `projects.yaml` 範例與 `README.md`：示範 slug 寫法（`repo_path: game-backend`）— 驗證：文件與範例一致
