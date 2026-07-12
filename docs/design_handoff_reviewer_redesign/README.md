# Handoff: Reviewer 後台重新設計 (Reviewer Admin Redesign)

## Overview
This is a full visual/IA redesign of the internal "Reviewer" tool — an admin dashboard for
scanning repos (incl. GitLab MRs) and tracking engineers' code-review activity and growth
over time. The redesign unifies a previously inconsistent style (mixed accent colors,
inconsistent radii/spacing, ad-hoc component patterns) into one coherent design system, and
restructures navigation and a few key screens (Reports Reader, Runs History, Dashboard
schedule panel) for better usability.

The original app is a Vite + vanilla TypeScript (DOM-manipulation) codebase. The stated goal
of this project is to re-implement it as **React (TypeScript) + Tailwind CSS**, broken into
reusable atomic components (Button, Card, Input, Badge, NavItem, StatCard, ListRow, Tabs, etc).

## About the Design Files
The file in this bundle — `Reviewer Redesign.dc.html` (+ its `support.js` runtime) — is a
**design reference prototype**, built in HTML to explore and validate the look, behavior, and
interactions. **It is not production code to copy directly.** Do not port its inline-style
authoring approach or its runtime as-is.

Your task is to **recreate this design in the target codebase's environment**: React +
TypeScript + Tailwind CSS, following the codebase's existing conventions (folder structure,
component patterns, linting, etc). Use Tailwind utility classes for all styling (translating
the design tokens below into your `tailwind.config` theme or arbitrary-value utilities), and
extract the repeated visual patterns into atomic, typed components as the user requested.

To view the reference: open `Reviewer Redesign.dc.html` in a browser (double-click, or serve
the folder over any static file server — no build step needed). It's a fully interactive
mock: sidebar nav switches views, list rows are selectable, tabs switch content, hover states
work, etc. There is no backend — all data is hardcoded mock data.

## Fidelity
**High-fidelity (hifi).** Colors, typography, spacing, radii, and component layout below are
final — recreate pixel-close using Tailwind. A few things are intentionally left as visual-only
stubs since there's no backend in this prototype: form "儲存"/"發布"/"送出" buttons, the MR
agent-chat thread, and the dashboard "立即執行" action don't call anything — wire these to your
real API layer (see original `api.ts`/`types.ts` for the existing endpoint/data shapes).

## Screens / Views
The app is a single-page shell: a fixed left sidebar (232px) + a scrollable main content area.
Navigation is entirely client-side state (no full reload). Six views:

### 1. Sidebar Navigation (persistent, all views)
- **Purpose**: primary navigation, grouped by function.
- **Layout**: 232px fixed-width column, white background, `1px solid #e2e8f0` right border,
  20px/14px padding. Vertical flex, `gap: 2px` between nav buttons.
- **Structure top→bottom**:
  1. Brand block: "Reviewer" (17px/700), connection status line below (12px, `#94a3b8`, with a
     6px green dot `#22c55e`) — "已連線 · G:/reviewer". Bottom border `1px solid #f1f5f9`,
     18px bottom padding, 14px margin-bottom.
  2. Group label "工作台" (10.5px/700, `#cbd5e1`, uppercase, letter-spacing 0.05em, padding
     `6px 10px 4px`).
  3. Nav buttons (in order): 控制台 (Dashboard) → MR 收件匣 (with a violet count badge,
     `#7c3aed` bg, white text, pill, 18px min-width) → 報告閱讀器 (Reports Reader, expands
     inline — see below) → 執行紀錄 (Runs History).
  4. Group label "設定" (same style as above, with a `1px solid #f1f5f9` top border + 14px
     top padding as a divider).
  5. Nav buttons: 專案設定 (Project Settings) → 人員設定 (People Settings).
  6. Footer (pushed to bottom via `margin-top: auto`): "Reviewer v0.4 · 內部工具", 11.5px,
     `#cbd5e1`, top border `1px solid #f1f5f9`.
- **Nav button style**: full width, no border, `border-radius: 8px`, `padding: 9px 12px`,
  13.5px, flex row `justify-content: space-between`. **Active**: bg `#eef2ff`, text `#4f46e5`,
  weight 600. **Inactive**: transparent bg, text `#334155`, weight 500.
- **報告閱讀器 expansion**: when this view is active, an inline sub-list appears directly
  below its button — one row per person, indented (`padding-left:10px; margin-left:10px`,
  left border `1px solid #f1f5f9`). Each row: name + an amber pending-count badge (only if >0
  open pending items), 12.5px, `border-radius:6px`, `padding:7px 8px`. Active/inactive colors
  match the main nav pattern. Clicking a person row switches both the active person AND
  navigates to the Reports view.

### 2. Dashboard (控制台)
- **Purpose**: at-a-glance health of the system; run reports on demand; edit the run schedule.
- **Header row**: h1 "控制台" (22px/700) + subtitle "上次執行 …" (13px, `#64748b`) on the
  left; primary button "▶ 立即執行" on the right (`#4f46e5` bg, white text, 8px radius,
  `10px 18px` padding, 13.5px/600).
- **Stat row**: 5 equal-width cards, `grid-template-columns: repeat(5,1fr)`, 12px gap. Each
  card: white bg, `1px solid #e2e8f0`, `border-radius:12px`, `padding:16px`. Label 12px
  `#64748b`, value 26px/600. Values: 專案, 工程師 (neutral `#0f172a`), 未讀報告 (`#4f46e5`),
  待確認 (`#d97706`), MR 草稿 (`#7c3aed`, this one is a clickable button → navigates to MR
  Inbox).
- **Second row**: 2-column grid (`1.2fr 1fr`), 14px gap: "最近報告" panel (list of person +
  project + read/pending status) and "最近執行" panel (recent run rows with status pill +
  trigger + timestamp, plus a "查看全部" link → Runs History).
- **排程設定 (Schedule) panel**: full-width card below the two-column row. Header row: "排程設定"
  title (14px/600) + "儲存排程" primary button (right-aligned). Body: 2-column grid (`1fr 1fr`,
  32px gap):
  - Left column "週報（軌道 1）": editable fields in a label+input grid
    (`grid-template-columns: auto 1fr; gap:12px 14px`): 啟用 (checkbox), 星期 (select),
    時間 HH:MM (text input), 時區偏移/分 (number), 專案逾時/秒 (number), 最大並發 (number).
    Below: next-run text + a green "✓ 排程器運行中" status pill (`#f0fdf4` bg, `#15803d` text).
  - Right column "MR 輪詢（軌道 2）": separated by a `1px solid #f1f5f9` left border + 32px
    left padding. One field: 間隔（分鐘） number input + hint text below.
  - Footer note (12px, `#94a3b8`) about which fields need a server restart to take effect.
  - **This whole panel replaces a cramped single-column 3rd-of-3 layout** — it was
    intentionally promoted to full width so the form isn't squeezed.

### 3. Project Settings (專案設定)
- **Purpose**: manage source repos (GitLab or local) and their MR-review policy.
- **Layout**: page title + "重新載入" button, then a bordered shell (`border-radius:12px`,
  `overflow:hidden`) split into a 260px list pane (white, right border) and a flexible detail
  pane (white, `padding:22px 26px`, must have `min-width:0` since it's a flex child).
- **List rows**: icon (see Assets) + name + trailing area that shows the last-report date by
  default, and swaps to a "▶ 執行" button on hover (implemented via onMouseEnter/onMouseLeave
  state — **not** CSS `:hover`, so it can conditionally replace content, not just restyle it).
  Both states must render at the same fixed height (22px) to avoid layout jitter.
- **Detail pane fields** (GitLab-source projects show all of these; local-source projects hide
  the GitLab-only fields): 來源類型 (GitLab/本地 segmented pills), Git Remote URL (read-only,
  monospace), 常駐分支 (read-only, monospace, comma-joined), MR 排除標籤, MR 必備標籤
  (optional), 儲存路徑 (read-only, monospace), 工程師對應 (read-only rows: avatar-initial
  circle + GitLab username (monospace, muted) + arrow + display name). Footer: 取消 / 儲存
  buttons, right-aligned, top border.
- Header actions (top-right of detail pane): "掃描 MR" (violet-tinted secondary button) and
  "移除" (red text, neutral border).

### 4. People Settings (人員設定)
- **Layout**: same 260px-list + flexible-detail shell as Project Settings.
- **Detail pane**: editable "顯示名稱" text input (15px/600, pre-filled, NOT a plain `<h2>` —
  this was a fix requested to make the display name editable) → "Identities" list (each row:
  kind + monospace value + 移除 button) → an "add identity" row below the list (kind `<select>`
  + value `<input>` + "＋ 新增" button, `flex-wrap:wrap` so it degrades instead of overflowing
  the panel on narrow widths) → "參與專案" rendered as a plain `<ul><li>` bullet list (was
  pill/chip badges before — changed per feedback) → 取消/儲存 footer.

### 5. Reports Reader (報告閱讀器)
- **Purpose**: read an individual's weekly report across the project(s) they touched, plus
  their longer-term growth trend. Person selection now lives in the **sidebar nav** (see
  above), not an in-page list — this was moved specifically to free up width for the report
  content itself.
- **Layout**: single card, `max-width:800px`, white bg, `border-radius:12px`,
  `padding:20px 24px`.
- **Header row**: 34px avatar circle (initial letter, `#eef2ff` bg / `#4f46e5` text) + name
  (17px/600) + report date (12px, `#94a3b8`) on the left; "📄 完整 md" (secondary button) and
  a 已讀/標記已讀 toggle button on the right (already-read state: green-tinted `#f0fdf4` bg /
  `#15803d` text / `#bbf7d0` border; unread: neutral secondary button).
- **Tabs row** (bottom-border `1px solid #e2e8f0`, tabs have `boxShadow: inset 0 -2px 0 #4f46e5`
  when active): **總覽 (Overview)** → one tab per project the person touched this period →
  **成長趨勢 (Growth Trends)**.
  - **總覽 tab**: intro paragraph (summary blurb, `#f8fafc` bg pill/box) → a responsive grid of
    per-project mini status cards (`repeat(auto-fit, minmax(160px,1fr))`; each shows project
    name + "活躍 · N 待確認" or "活躍 · 無待確認", amber if N>0 else muted) → "本週重點"
    bullet list (each bullet prefixed with **bold project name**, merged across all of the
    person's projects) → "成長面向" bullet list (same merge pattern) → if there are any open
    (unresolved) pending items anywhere, an amber callout box ("⚠ 待確認彙整（1on1 時詢問）",
    `#fffbeb` bg, `#fde68a` border, `#92400e`/`#78350f` text) grouping open questions by
    project, each with a checkbox.
  - **Per-project tab**: that project's 亮點 (highlights) / 成長觀察 (growth) bullet lists,
    plus a plain checkbox list of ALL its pending items (open + resolved, checkbox reflects
    resolved state) — no callout box here, that's overview-only.
  - **成長趨勢 tab**: a long-term observation paragraph + a "成長時間軸" timeline (month +
    content rows, divided by `1px solid #f1f5f9`).

### 6. MR Inbox (MR 收件匣)
- **Purpose**: review/edit/publish AI-drafted MR review comments; chat with the review agent
  to refine a draft.
- **Layout**: title + subtitle, then `grid-template-columns: 320px 1fr`, min-height 560px.
- **Left (list)**: card with a 3-tab filter header (草稿/已發布/已忽略 — violet active
  underline `inset 0 -2px 0 #7c3aed`, active text `#6d28d9`), then a scrollable list of MR rows
  (title, `project · !iid · author` meta line). Active row: `#f5f3ff` bg + `inset 3px 0 0
  #7c3aed` left accent (this left-accent-on-selection is a deliberate selection indicator, not
  a decorative container border — don't apply it to static cards elsewhere).
- **Right (detail)**: MR title + meta + a violet "reviewer_agent" pill, a monospace draft-body
  `<textarea>` (**must use `defaultValue`, not children text — putting the string between
  `<textarea>` tags renders literally as "[object Object]" or fails to update in React**),
  action row (忽略 / 儲存草稿 secondary buttons + 發布 primary violet button), then a chat
  panel below a divider: message bubbles (user messages right-aligned `#ede9fe`, agent replies
  left-aligned `#f1f5f9`) + a textarea/send-button input row.

### 7. Runs History (執行紀錄)
- **Layout**: `grid-template-columns: 300px 1fr`, min-height 560px.
- **Left (list)**: scrollable card of run rows (status pill + `#id trigger label`, started_at +
  duration, project count + skipped count). Active row: `#eef2ff` bg + `inset 3px 0 0 #4f46e5`.
- **Right (detail)**: "Run #id" (18px/700) + status pill header, then a meta grid using
  `grid-template-columns: repeat(auto-fit, minmax(110px,1fr))` (**not** a fixed
  `repeat(5,1fr)` — that overflowed/wrapped badly at narrower widths; auto-fit lets it degrade
  to 2 rows gracefully) showing 觸發/開始/結束/耗時/專案. Below: "專案結果" — one card per
  project in the run (name + state pill + duration), and if a project had MR skips, a nested
  "MR Skip 摘要" section inside that card (reason label × count, then the skipped MR numbers)
  with a red-tinted card style (`#fef2f2` bg, `#fecaca` border) instead of the default
  `#f8fafc`/`#e2e8f0`.

## Interactions & Behavior
- All navigation is client-side React state (a `view` string) — no routing library needed
  unless the target codebase already uses one, in which case map each view to a route.
- List/row selection (project, person, MR, run) is local state holding the selected id/name;
  clicking a row updates it and the detail pane re-renders.
- Hover-to-reveal-a-button (Project Settings list rows) needs a real hover state (React
  `onMouseEnter`/`onMouseLeave` or CSS `:hover` + a sibling toggle), not just a color change —
  it swaps the trailing content entirely. Keep both states the same height.
- Tabs (Reports Reader, MR Inbox filter) are simple active-key state, no animation needed beyond
  an instant underline/background swap.
- No loading/error states are mocked in this prototype (no real backend calls) — design these
  per the target app's existing conventions (spinners exist in the original app's `.project-list-spinner`
  class if you want to preserve that treatment for the "running" project state).
- Not responsive below ~1024px in this prototype (it's an internal admin tool) — but note the
  two `min-width:0` / `auto-fit` fixes above, which specifically prevent overflow at moderately
  narrow widths and should be preserved.

## State Management
Minimal, all local component/page state — no global store needed:
- `view`: which of the 6 top-level views is active.
- `selectedProjectName`, `hoveredProjectName` (Project Settings).
- `selectedPersonId` (People Settings), `reportPersonId` + `reportProjectTab` (Reports Reader,
  reset to `'overview'` whenever the person changes).
- `mrFilter` (draft/published/ignored) + `selectedMrId` (MR Inbox).
- `selectedRunId` (Runs History).
All of the above should ultimately be backed by real data fetches (see original `api.ts`) —
in this prototype they're hardcoded arrays in the component.

## Design Tokens

**Typography**: Inter (400/500/600/700) for UI text, JetBrains Mono (400/500) for
code/identifiers/paths — both loaded from Google Fonts. Base body size 14px.
- Page title (h1): 22px/700, letter-spacing -0.01em
- Section/card title (h2): 17px/600
- Panel eyebrow labels: 12–12.5px/600–700, `#64748b`, uppercase, letter-spacing 0.03–0.04em
- Body text: 13–13.5px
- Small/meta text: 11–12px, `#94a3b8`
- Stat values: 26px/600

**Colors** (light theme, cool-neutral slate scale):
- Background: `#f8fafc` (page), `#ffffff` (cards/sidebar)
- Borders: `#e2e8f0` (default), `#f1f5f9` (subtle dividers)
- Text: `#0f172a` (primary), `#334155` (secondary), `#64748b` (muted labels), `#94a3b8` (meta),
  `#cbd5e1` (faint)
- **Primary accent (single)**: indigo `#4f46e5` / tint `#eef2ff` / dark `#4338ca` — used for all
  primary actions, active nav/tab/list-row states, and links.
- **Semantic colors** (status only, not decorative): success/read green `#15803d` on `#f0fdf4`;
  warning/pending amber `#d97706` on `#fffbeb`; danger/skip red `#dc2626` on `#fef2f2`; MR-track
  violet `#7c3aed` / `#6d28d9` on `#f5f3ff`/`#ede9fe` (reserved specifically for the MR
  review/inbox feature track, not used elsewhere).

**Radii**: 6px (small chips/selects), 8px (buttons, inputs, small badges), 10px (mid cards),
12px (panels/shells/major cards), 999px (pills/badges).

**Spacing**: sidebar padding 20px/14px; page padding 32px/40px; card padding typically
16–24px; row gaps 6–14px; grid gaps 12–16px (content), 32px (schedule panel's 2 columns).

**Shadows/borders**: flat design — no drop shadows anywhere. All elevation/grouping is done
with `1px solid` borders + background color, plus the `inset 3px 0 0 <accent>` selection
accent on active list rows (MR Inbox, Runs History) and `inset 0 -2px 0 <accent>` on active
tabs.

## Assets
- **Fonts**: Google Fonts `Inter` (400,500,600,700) and `JetBrains Mono` (400,500).
- **Source-type icons** (Project Settings): inline SVG data-URIs, color-matched to
  active/inactive state (`#4f46e5` active / `#94a3b8` inactive), swapped via `background-image`:
  - GitLab icon (gitlab-source projects) — provided by the team, a 256×256 viewBox path.
  - Folder icon (local-source projects) — provided by the team, a 24×24 viewBox path.
  Both data URIs are inlined directly in the component (see `sourceIconStyle()` /
  `gitlabIconDataUri()` / `folderIconDataUri()` in the reference file) — copy them verbatim
  into your icon component rather than sourcing a different icon set, since they were supplied
  by the design team specifically for this purpose.
- No other custom icons/images — everything else is typographic or simple shape/color coding
  (status pills, dots, avatar-initial circles).

## Files
- `Reviewer Redesign.dc.html` — the full interactive reference (all 6 views + nav). Open
  directly in a browser alongside `support.js` (same folder) to view/interact with it.
- `support.js` — runtime the reference file needs to render; not relevant to your React
  implementation, just required to preview the reference itself.
