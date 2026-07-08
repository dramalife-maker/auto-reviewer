# 人物層長期觀察遷移指引

本文件說明如何將舊 1on1 筆記或跨專案觀察放入新的人物層目錄，供趨勢 Tab 顯示。

## 目錄位置

跨專案長期觀察統一放在：

```text
$DATA_ROOT_DIR/reports/_people/{display_name}/
```

其中 `{display_name}` 必須與系統中 `people.display_name` **完全一致**（與週報目錄命名相同）。

範例：

```text
G:/reviewer/reports/_people/Alice Chen/index.md
```

## 可放置的檔案

| 檔案 | 用途 |
|------|------|
| `index.md` | 長期觀察（跨專案綜合敘事） |
| `YYYY-MM.md` | 月度成長軌跡素材 |
| `_notes.md` | 歷史待確認（每行 `- [YYYY-MM] 問題文字`） |

## 寬鬆格式

`index.md` **不需要** YAML frontmatter，也**不需要**轉成 `summary.md` 契約格式。可直接貼上自由 Markdown 舊筆記，後端會全文回傳至趨勢 API 的 `long_term_observation` 欄位。

週報 `summary.md` 的入庫規則不變：仍須符合 `output-contract.md`，且僅存在於專案層：

```text
$DATA_ROOT_DIR/reports/{project_name}/{display_name}/{YYYY-MM-DD}/summary.md
```

## 與專案層檔案的關係

### `index.md`

- **人物層** `reports/_people/{display_name}/index.md`：趨勢 Tab 讀取的主資料源（跨專案）。
- **專案層** `reports/{project}/{display_name}/index.md`：可選的單專案技術脈絡；趨勢 API **不會**讀取。

### `YYYY-MM.md`（兩層皆須維護）

| 層級 | 路徑 | 用途 |
|------|------|------|
| 專案層 | `reports/{project}/{display_name}/YYYY-MM.md` | 該人在**此 repo** 當月的成長（技術、交付、review） |
| 人物層 | `reports/_people/{display_name}/YYYY-MM.md` | **跨專案**當月成長綜合；趨勢 Tab「成長軌跡」讀此檔 |

週報 `summary.md` 的 `## 成長面向` 是本週切片，不能取代月檔。workflow 每週會追加兩層月檔；人物層應綜合各專案重點，而非複製專案層月檔全文。

既有專案層 `index.md` / `YYYY-MM.md` 可保留；跨專案內容請整理到人物層對應檔案。

## 改名注意

若修改 `people.display_name`，需手動將 `reports/_people/` 下對應目錄重新命名，否則趨勢 API 會找不到檔案。

## 自動維護

成功執行「全部執行」後，`reviewer-batch` workflow 會：

- 追加 `reports/{project}/{display_name}/YYYY-MM.md`（本專案月度成長）
- 追加 `reports/_people/{display_name}/` 下 `index.md`、`YYYY-MM.md`、`_notes.md`（跨專案綜合）

手動放置的內容會被 workflow **追加**段落，不會整檔覆寫。
