# Design review: palette × layout HEAD fields

- **Platform:** CLI / TUI (cc-pulseline statusline)
- **Primary job:** confirm theme palette covers the new L1 fields, audit cross-layout label vocabulary, and decide whether HEAD-row colors should follow themes.
- **Reference:** existing `ThemePalette` struct (`src/render/color.rs`), `docs/theme-palette.md`, the 5 frame implementations under `src/render/frames/`.
- **Source reviewed:** `src/render/{color.rs,layout.rs,pane.rs,frames/*.rs}`, `src/config.rs`, `docs/theme-palette.md`. Most recent feature commit: `5b6964e` (CC 2.1.119+ alignment — adds `show_effort`, `show_thinking`).
- **Date:** 2026-05-01

---

## 1. 新 HEAD 欄位 vs Theme palette 覆蓋

### 目前 L1 / Identity 列上的欄位

| Toggle | 顯示 | 取色來源 | Palette-aware? |
|---|---|---|---|
| `show_model` | `M:Opus` | `p.stable_blue` | ✅ 直接欄位 |
| `show_effort` *(new)* | `E:high` | `p.color_for_effort_level(level)` → `structural` / `stable_blue` / `active_amber` / `active_coral` / `alert_red` / `secondary` | ✅ **語意函式** — 最佳實踐 |
| `show_thinking` *(new)* | `[T]` (label-only pill) | `p.active_purple` 直接讀取 | ⚠️ palette-aware,但**沒有語意 alias** |
| `show_agent` *(new)* | `AG:greg-bot` | `p.stable_blue` 直接讀取 | ⚠️ palette-aware,但**和 M: 撞色** |
| `show_style` | `S:explanatory` | `p.secondary` | ✅ |
| `show_version` | `CC:2.1.119` | `p.secondary` | ✅ |
| `show_project` | `P:~/repo` | `p.secondary` | ✅ |
| `show_git` + `show_git_stats` | `G:main*↑1` | `p.git_*()` 語意函式 | ✅ 語意函式 |
| `show_worktree` | `(WT)` (附在 git 後面) | 借用 git 顏色 | ✅ |

### 結論
- **沒有任何 HEAD 欄位寫死 ANSI 碼或硬編色號** — 都已經跑進 palette。換 theme 不會壞。
- 但 `effort` 是「設立語意 alias」的乾淨範本 (`color_for_effort_level`),`thinking` 和 `agent` 沒享受到同等待遇 — 它們直接讀 `p.active_purple` / `p.stable_blue`,**theme 作者沒辦法只移動 thinking 或 agent 的色相而不影響其他用該欄位的元素**。

### 衝突點

1. **`AG:` 和 `M:` 都是 `stable_blue`** — L1 上目視幾乎分不開:
   ```
   M:Opus | AG:greg-bot | S:...
   ──── 兩個藍色標籤連在一起,只能靠 sep 區隔
   ```
   而 L5+ 的 agent 列(`A:Explore`)用的是 `active_purple`。同一個概念(agent identity),L1 和 L5+ 用不同色 → 不一致。

2. **`[T]` thinking pill 和 L5 agent 同樣 `active_purple`** — agent 是動態的、會跑;thinking 是靜態的「mode 開了」標記。共用色但語意層級不同。

---

## 2. CFG vs ENV 用詞稽核

### 兩套並存的命名系統

```
                Macro 4-group           Per-metric TAG
                (zones/grid/sections    (ledger only)
                 /console PaneGroup)
   L1 Identity  "Identity"              (升到 frame title,無 tag)
   L2 Config    "Config"        ←→      "ENV"     ⚠️ 同一列,兩個名字
   L3 Budget    "Budget"        ←→      "CTX" / "TOK" / "COST" / "5h" / "7d"
   L4+ Activity "Activity"      ←→      "TOOL" / "AGENT" / "TODO"
```

### Findings

- **L2 兩個名字最明顯**:macro 系統叫 `Config`,ledger TAG 叫 `ENV`。同一份資料(CLAUDE.md / rules / memories / hooks / MCPs / skills)。
- **底層 provider 是 `env.rs` / `EnvCollector`** — 內部一直叫 ENV,UI 卻一半叫 Config 一半叫 ENV。
- **Budget 沒這個問題,但理由不同** — macro 把 CTX+TOK+COST+quota 合成一個 group label,ledger 是「拆開 per-metric」。這是**設計上的分歧,不是命名上的不一致**(每個 layout 在密度光譜上選不同點)。
- **真正可以收斂的只有 L2** — 因為 ledger 的 `ENV` 沒有「拆開」,就一個 row。

### 推薦

挑一個詞,改另一個。**選 `ENV`** 比較合適,理由:

| 比較 | `Config` | `ENV` |
|---|---|---|
| 對齊 provider 名稱 | ✗ (`EnvCollector`) | ✓ |
| 字數 | 6 | 3,和 CTX/TOK/COST 韻律一致 |
| 和 TOML 設定撞名 | ✗ `--print config` 會混淆 | ✓ |
| 對使用者意涵 | 偏設定檔 | 偏「環境掃描結果」(含 hooks/MCPs/skills) |

→ 把 `src/render/layout.rs:161` 的 `"Config"` 改成 `"ENV"`,順便把 `LineKind::Config` 重新命名為 `LineKind::Env`(or 保留 enum 名稱,只動 user-facing label)。

---

## 3. 新 HEAD 欄位是否能參考 theme 顏色做變化?

**Yes,而且現在就已經是這樣 —** 但有 3 個 polish 機會。

### Adjustments

#### Must

**M1. `Config` → `ENV` 統一** (lens: Consistency)
- 現況:同一列在 ledger 叫 ENV,在其他 4 個 layout 叫 Config。
- 動作:`src/render/layout.rs:161` 把 PaneGroup label 改 `"Config"` → `"ENV"`(或用設定檔可覆寫的常數)。也更新 `docs/theme-palette.md` line 411 區的 ASCII art。
- 驗收:`cargo test`,並抽一個 grid + 一個 ledger 截圖比對 label 一致。

#### Should

**S1. 為 `agent` 和 `thinking` 加語意 alias** (lens: Affordance + Theme separability)
- 現況:`format_line1` 直接寫 `p.stable_blue`(agent)、`p.active_purple`(thinking)。Theme 作者要把 thinking 從紫色換成琥珀色,得連動所有 `active_purple` 用法。
- 動作:在 `ThemePalette` 加兩個語意函式:
  ```rust
  pub fn agent_identity(&self) -> &str { &self.active_purple }   // L1 AG: + L5 A:
  pub fn thinking_pill(&self) -> &str  { &self.active_amber }    // 或 dedicated 新欄位
  ```
- 預期效果:`AG:greg-bot` 在 L1 變紫色,和 L5 `A:Explore` 顏色一致;thinking 從紫色獨立出來,不再和 agent 撞色。
- 驗收:更新 `docs/theme-palette.md` Line 1 ASCII annotation;新增 `ThemePalette` test 確保兩個 alias 在 tokyo-night dark 下分別 = 183 / 178。

**S2. L1 上 `M:` 和 `AG:` 撞色** (lens: Hierarchy)
- 現況:兩者都 `stable_blue`,L1 「主要身份 vs session agent」的層級被壓平。
- 動作:跟 S1 合併 — `AG:` 改用 `agent_identity()`(紫色),`M:` 維持 `stable_blue`。Model 是 primary identity,agent 是 secondary identity,兩種藍綠紫的層級就出來。
- 驗收:`tests/agent_worktree.rs` 加一條 assertion 檢查 AG: 顏色 ≠ M: 顏色。

#### Could

**C1. 為 `effort` 開放 theme 微調入口** (lens: Theme expressiveness)
- 現況:`color_for_effort_level` 用內建語意對照(low→structural / medium→stable_blue / ...)。Theme 作者想讓 `xhigh` 改用其他色就只能改全域 `active_coral`。
- 動作:在 theme JSON 開放 `effort_low / effort_medium / effort_high / effort_xhigh / effort_max` 欄位(都 `Option<u8>`,預設 fallback 到目前 mapping),套到 palette。
- 驗收:加 1 個 custom theme,把 `effort_max = 196` 改 `effort_max = 124` 看顏色變化。
- 為什麼是 Could: 目前 5 階梯 mapping 已經涵蓋大部分 theme 的視覺語意 (低→中→暖→警告),YAGNI 直到有人實際提出。

**C2. 把 ledger TAG 顏色綁到資料種類** (lens: Affordance)
- 現況:`framed_tag_row` 統一給 TAG 上 `secondary` 色,所有 TAG(ENV/CTX/TOK/COST/TOOL/AGENT/TODO)都長一樣。
- 動作:讓 TAG 取它自己 row 的主色 — `CTX` 用 `color_for_ctx_pct`,`AGENT` 用 `agent_identity()`,`TOOL` 用 `tool_blue`,`COST` 用 `color_for_burn_rate`。TAG 變成「色塊化的圖例」,row 內容變化時 TAG 也呼吸。
- 為什麼是 Could: 會讓 ledger 從「靜態列表」變「動態儀表板」,有些使用者可能反而覺得吵。先開個 TOML toggle 試水溫(`ledger.dynamic_tag_color = false` default)。

---

## Cross-cutting recommendation

**新增 HEAD 欄位的 checklist**(寫進 `.claude/rules/rendering.md` 那批文件):

1. 顏色從 `&ThemePalette` 拿,不能寫 const
2. **如果這個欄位代表的「概念」在其他 line 也有出現**(像 agent 在 L1 和 L5),建一個 `pub fn xxx(&self) -> &str` 語意 alias
3. **如果這個欄位的色階會根據資料變化**(像 effort、cost、ctx),建一個 `pub fn color_for_xxx(...)` 函式,而不是在 `format_line1` 裡用 if/else
4. 在 `docs/theme-palette.md` Line 1 ASCII annotation 加一行
5. 給 `tests/` 加一條 assertion,確認 tokyo-night dark 和 light 兩個 variant 都會印出非空字串

---

## Next step

選一條動:S1(agent + thinking 語意 alias)落地最快、回報最高;若想一次清,把 M1 + S1 + S2 包成一個 PR — 都動 `layout.rs` + `color.rs` + `theme-palette.md`,改動範圍剛好同心。

---

# Round 2 — 後續對話補充 (2026-05-01)

## Q: 是否需要 per-layout palette?

**不需要,但要加 palette 欄位。**

- Per-layout palette(每個 layout 一份顏色)= **錯方向**。10 個 theme × 5 個 layout = 50 套色要設計,沒人會做完,跨 layout 視覺一致性反而瓦解。
- 現在的單一 `ThemePalette` 不會「自動爆」,但會慢慢漂 — 每次新 layout 引入新角色(ledger TAG 欄、console frame title),都會借一個現有欄位 → 該欄位被多義化 → theme 作者再也沒辦法獨立調整。
- **已經有兩個漂移點:**
  1. `framed_tag_row` 直接讀 `p.secondary`(ledger TAG 欄借了 L1 secondary 用)
  2. `format_line1` 直接讀 `p.active_purple`(thinking pill)和 `p.stable_blue`(AG:)

### 既有先例

`strata_state` / `strata_activity` 是「frame chrome 角色,獨立色位,Optional 帶 fallback」的範本。`aurora_low/mid/high` 是「sparkline gradient 角色,獨立色位,Optional 帶 fallback」的範本。同一個機制再用一次。

## 落地計畫(M1 + S1 + S2 + Q3 directive 整合)

### 新增 3 個 palette 欄位

| 新欄位 | 用途 | Fallback(Option = None 時) | 為什麼是新欄位而非 alias |
|---|---|---|---|
| `tag_label` | Ledger TAG 欄(ENV/CTX/TOK/COST/...) | `secondary` | TAG 欄出現在每一行,L1 secondary 出現在 1 行;頻率不同,色重應該分開可調 |
| `head_agent` | L1 `AG:greg-bot` | `active_purple` | 與 L5+ `A:Explore` 同一概念(agent identity),共用語意但 theme 作者可能想 L1 比 L5 更收斂 |
| `head_thinking` | L1 `[T]` pill | `active_amber` | thinking ≠ agent;借 `active_purple` 會撞色,借 `active_amber` 又綁住 effort medium-high |

每個欄位都走 `Option<u8>` + `#[serde(default)]`,custom theme 漏寫 → 落 fallback + warn-once,和 `strata_*` 一致。

### 新增 3 個 semantic 函式

```rust
impl ThemePalette {
    pub fn tag_label(&self) -> &str { &self.tag_label }
    pub fn head_agent(&self) -> &str { &self.head_agent }
    pub fn head_thinking(&self) -> &str { &self.head_thinking }
}
```

(欄位/函式同名 — Rust 允許,跟 `strata_state` 同模式。)

### 改動清單

**`src/render/color.rs`**
- `ThemePalette` 加 3 個 `String` 欄位
- `PresetColors` 加 3 個 `Option<u8>` 欄位(`#[serde(default)]`)
- `LightEmphasis` 同步加(讓 light variant 可獨立指定)
- `build_palette` 加 fallback:`tag_label → secondary`、`head_agent → active_purple`、`head_thinking → active_amber`
- `apply_color_overrides` 加 3 條 `apply!`
- `warn_*_fallback_once` 模式加一個(或合併成一個 generic warn)
- 加 `pub fn builtin_*_for(name, variant)` 給 contrast lint 用(如果有)

**`src/config.rs`**
- `ColorsConfig` 加 3 個 `Option<u8>`

**`src/render/layout.rs`**
- L160-172 `pane_config_from`:`"Config"` → `"ENV"`(M1)
- `format_line1` AG::`&p.stable_blue` → `p.head_agent()`(S2)
- `format_line1` `[T]` pill:`&p.active_purple` → `p.head_thinking()`(S1)

**`src/render/frames/ledger.rs`**
- `framed_tag_row` line 259:`&ctx.p.secondary` → `ctx.p.tag_label()`

**`src/themes/*.json`**(10 個 built-in)
- 每個 theme 加 3 個欄位到 `palette_mapping`,並在 `light_emphasis` 視需要 override
- ⚠️ **這是設計工作,不是 boilerplate** — 每個 theme 要按自己的 palette 故事決定數值。例如:
  - `cyberdeck-hud`(neon):`head_thinking` 可以選比 amber 更亮的霓虹色 → 配合整體高飽和度
  - `titanium-precision`(desaturated):`tag_label` 應該比 secondary 更弱(把 ledger TAG 欄壓成「次要結構元素」),`head_thinking` 用接近 active_coral 的濁色
  - `tokyo-night`(reference):安全值就是 fallback(146 / 183 / 178)
- **不要全部塞 fallback 值** — 那等於沒設計

**`docs/theme-palette.md`**
- 加 3 個欄位的說明 + role rationale
- 更新 Line 1 ASCII annotation(把 AG: 從 stable_blue 改 active_purple,加 [T] head_thinking 標註)
- 更新 ledger 範例的 TAG 顏色標註
- L2 由 "Config" 改 "ENV" 的小段(macro 4-group 的 group label)

**測試**
- `tests/agent_worktree.rs` 加 `AG:` 色 ≠ `M:` 色 assertion
- `src/render/color.rs` mod tests:tokyo-night dark 下 `head_agent != head_thinking`(避免兩者撞色 regression)
- `tests/cli_flags.rs` 或既有 ledger 測試:把 ENV TAG 色從 secondary 解耦的 assertion(ledger 套 custom theme,只覆寫 `tag_label`,確認 secondary 不受影響)
- Ascii catch-net:不影響(這幾個變動不引入新 Unicode glyph)

### 建議分 commit

可一個 PR 三個 commit,降 review 負擔:

1. **`feat(palette): add tag_label / head_agent / head_thinking fields`**
   - 純加欄位 + fallback + semantic 函式 + override apply
   - 渲染端不動,所有 layout 視覺零變化
   - Tests:palette unit tests(欄位存在 + fallback 行為 + override 套用)
2. **`feat(layout): rewire L1 AG/thinking + ledger TAG to new palette fields; rename Config→ENV`**
   - M1 + S1 + S2 主刀
   - Built-in theme JSON 暫不動,全部走 fallback(視覺只在 AG: 紫化、thinking 變琥珀色)
3. **`feat(themes): per-theme designed values for tag_label / head_agent / head_thinking`**
   - 10 個 JSON 各自設計
   - 把 Round 2 表格的設計理由寫成 commit message / PR body

如果只想一個 commit,順序也是這樣 — 先擴 palette,再 rewire,最後 polish theme 數值。

### 不做的事

- ❌ 不為 console 的 frame title 拆色位 — 它直接複用 L1 identity 段(model+path+branch),沒新角色。
- ❌ 不為 zones 的 `──── activity ────` rule 拆色位 — 已經用 `secondary`,語意對齊「分節標籤」,不衝突。
- ❌ 不為每個 layout 拆 cost / quota / ctx 顏色 — 這些角色已經有自己的色階(`color_for_burn_rate` 等),跨 layout 一致是優點不是 bug。
- ❌ 不在這個 PR 動 `effort_*` 拆位(C1) — 等真實使用者要求,YAGNI。

## Updated next step

```
PR 1: palette 欄位擴張 (commit a)        ──┐
PR 1: rewire + ENV 改名 (commit b)        ──┼─── 同一 PR
PR 1: 10 個 theme JSON 設計 (commit c)    ──┘
```

預估:commit a 30 分鐘、commit b 20 分鐘、commit c 1-2 小時(設計 10 個 theme 的 3 個新欄位數值是慢的部分)。
