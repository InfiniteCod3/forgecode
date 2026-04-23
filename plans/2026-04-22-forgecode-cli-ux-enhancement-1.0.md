# ForgeCode CLI — Enhanced Terminal UX/UI Plan

## Objective

Modernize and elevate the terminal user experience across ForgeCode's CLI by improving token usage display, spinner/status indicators, title/status line formatting, information panels, progress feedback, and the overall visual polish of text-based output. The current UI is functional but plain — this plan proposes concrete, incremental improvements to make the CLI feel more professional, informative, and visually appealing.

---

## Current State Assessment

### Key Files and Components

| Component | Primary File | Description |
|-----------|-------------|-------------|
| Spinner / progress | `crates/forge_spinner/src/lib.rs` | Braille-spinner with random words ("Thinking", "Processing", etc.) |
| Progress bar | `crates/forge_spinner/src/progress_bar.rs` | Basic `indicatif` progress bar |
| Right prompt (ZSH) | `crates/forge_main/src/zsh/rprompt.rs` | Shows agent, tokens, cost, model |
| REPL prompt | `crates/forge_main/src/prompt.rs` | Two-line left/right prompt with nerd font icons |
| Title/status lines | `crates/forge_main/src/title_display.rs` + `crates/forge_domain/src/chat_response.rs` | `● [HH:MM:SS] Title` with category-based colors |
| Info panels | `crates/forge_main/src/info.rs` | Key-value layout with sections, green bold keys |
| Token usage | `crates/forge_main/src/info.rs:470-500` | Plain text TOKEN USAGE section |
| Streaming renderer | `crates/forge_main/src/stream_renderer.rs` | Wraps `StreamdownRenderer` with spinner pause/resume |
| Banner | `crates/forge_main/src/banner.rs` | ASCII art box with tips |
| Display constants | `crates/forge_main/src/display_constants.rs` | Centralized `[yes]`, `[no]`, `[empty]`, `[built-in]` markers |
| Todo format | `crates/forge_app/src/fmt/todo_fmt.rs` | Nerd-font checkbox icons with status colors |
| Chat response handler | `crates/forge_main/src/ui.rs:3890-4004` | Routes streaming events to writer, spinner, title lines |

### Identified Pain Points

1. **Token usage is a static block** — shown only via `:usage` or after conversations end, with no real-time awareness or visual progression.
2. **Spinner messages are random words** — no context about what phase the AI is in (thinking vs. tool execution vs. generating).
3. **Status/title lines are monochrome** — only bullet color varies by category; sub-titles and timestamps are dimmed but lack structural visual hierarchy.
4. **No progress indication for multi-tool turns** — when the AI calls multiple tools sequentially, there's no visible count (e.g., "Tool 2/5").
5. **Info panels lack visual grouping** — sections blend together; no separators, borders, or indentation hierarchy.
6. **Token prompt display is binary** — either active (bright) or inactive (dim) with no transition animation or incremental counter.
7. **`Info` display for metrics/file changes is minimal** — plain text `−N +M` without visual emphasis on the numbers.
8. **Banner tips are static** — always the same list regardless of user experience level.

---

## Implementation Plan

### Phase 1: Enhanced Token Usage Display (Real-Time Awareness)

- [ ] **1.1** Add upward/downward token delta indicators to the right prompt. When token count increases between prompts, show a green `▲` icon with the delta (e.g., `▲1.2k`). When it stays flat, show nothing. When the conversation is compacted and tokens drop, show a yellow `▼` with the delta. This requires storing the *previous* token count in `ForgePrompt` and computing the delta in `render_prompt_right()`.

  Rationale: Users currently see a raw number with no sense of trajectory. Showing directionality gives immediate feedback on context growth.

  Key files: `crates/forge_main/src/prompt.rs:105-176` (right prompt render), `crates/forge_main/src/zsh/rprompt.rs` (ZSH rprompt), `crates/forge_domain/src/context.rs:637-651` (token count tracking).

- [ ] **1.2** Add human-readable token formatting with contextual color. Instead of always showing `1,234` format, use color to indicate utilization approach: green (low), yellow (moderate), red (high) relative to known context window size. This requires plumbing the context window size from the model info into `ForgePrompt`.

  Rationale: A raw number has no frame of reference. Color-coding against the context limit gives users an at-a-glance sense of how much headroom they have.

  Key files: `crates/forge_main/src/prompt.rs:130-147`, `crates/forge_api` (model context_length info).

- [ ] **1.3** Add a token utilization progress bar to the `:usage` command output. When the model's context window size is known, render a compact horizontal bar like `[████░░░░░░] 42%` alongside the numeric values, using colored blocks to show input vs. output ratio.

  Rationale: The current `:usage` command is a plain key-value list. A visual bar communicates proportionality faster than numbers.

  Key files: `crates/forge_main/src/info.rs:470-500` (`From<&Usage>` impl), `crates/forge_spinner/src/progress_bar.rs` (existing bar style reference).

- [ ] **1.4** Accumulate and display per-turn token deltas under `:usage`. Show both the running total and a breakdown of the last N turns, so users can see which requests consumed the most tokens.

  Rationale: Users need spending awareness without external dashboards.

  Key files: `crates/forge_main/src/ui.rs:4143-4171` (`on_usage`), `crates/forge_domain/src/conversation.rs` (per-message usage data).

### Phase 2: Contextual Spinner Improvements

- [ ] **2.1** Replace random spinner words with phase-aware messages. Instead of randomly cycling through ["Thinking", "Processing", "Analyzing", ...], use a deterministic message based on the current phase:
  - LLM call in progress → `"Thinking"` (with elapsed timer)
  - Tool executing → `"Running {tool_name}"` (when available from `ChatResponse::ToolCallStart`)
  - Retrying → `"Retrying (attempt N)"` (from `ChatResponse::RetryAttempt`)
  - Compacting → `"Compacting"`
  
  This requires updating `SpinnerManager::start()` to accept the phase context, and updating `handle_chat_response` to call `spinner.set_message()` at each phase transition.

  Rationale: Random words are confusing and provide no real information about what's happening.

  Key files: `crates/forge_spinner/src/lib.rs:72-133`, `crates/forge_main/src/ui.rs:3890-4004` (chat response handler).

- [ ] **2.2** Add a tool enumeration counter to the spinner prefix. When multiple tools are called in sequence, show `Tool 1/N` in the spinner message. Track tool call count in the UI state and update it on each `ChatResponse::ToolCallStart`.

  Rationale: Multi-tool turns currently show no progress — users don't know if one more tool is coming or ten.

  Key files: `crates/forge_main/src/ui.rs:3912-3934` (ToolCallStart handler), `crates/forge_main/src/state.rs` (UI state).

- [ ] **2.3** Animate the spinner tick characters to pulse on phase change. When transitioning from "Thinking" to "Running tool_name", briefly flash the spinner color (e.g., green → cyan) for 500ms before settling to the new phase color. Add a `Style` parameter to `SpinnerManager::start()` that maps `Category` colors to spinner widgets.

  Rationale: A subtle color flash signals phase transitions even when the user isn't reading the text.

  Key files: `crates/forge_spinner/src/lib.rs:100-117` (progress style), `crates/forge_domain/src/chat_response.rs:133-140` (`Category` enum).

### Phase 3: Title/Status Line Visual Hierarchy

- [ ] **3.1** Redesign title format with structured visual prefixes per category:
  - `Action` → `▶` (bright cyan) instead of `●` (yellow)
  - `Info` → `ℹ` (blue) instead of `●` (white)
  - `Debug` → `┆` (dim gray) instead of `●` (cyan)
  - `Error` → `✗` (bold red) instead of `●` (red)
  - `Completion` → `✓` (bold green) instead of `●` (yellow)
  - `Warning` → `⚠` (bright yellow) — already used, no change needed

  Rationale: The current `●` bullet is identical for all categories, forcing users to read the color to distinguish type. Distinct glyphs provide instant category recognition.

  Key files: `crates/forge_main/src/title_display.rs:26-35` (icon mapping), `crates/forge_domain/src/chat_response.rs:132-140` (`Category` enum).

- [ ] **3.2** Add category-colored sub-title formatting. Currently sub-titles are uniformly dimmed. Instead, format them as `subtitle` with a subtle separator like ` — ` and render the sub-title text in the category's accent color (dimmed version).

  Rationale: Sub-titles carry useful secondary info (conversation IDs, completion markers) that currently fade into the background.

  Key files: `crates/forge_main/src/title_display.rs:54-58` (sub-title formatting), `crates/forge_domain/src/chat_response.rs:142-149` (`TitleFormat` struct).

- [ ] **3.3** Add a completion summary bar after `TaskComplete`. When a task finishes, output a compact one-line summary showing: duration, tools called, token delta, cost. Format: `✓ Done in 1:23m · 3 tools · ▲2.1k tokens · $0.04`. This data is already available from the conversation's metrics and usage.

  Rationale: Currently `TaskComplete` just prints `● [HH:MM:SS] Finished conversation_id` — no summary of what was accomplished.

  Key files: `crates/forge_main/src/ui.rs:3990-4001` (TaskComplete handler), `crates/forge_main/src/info.rs:417-468` (Metrics → Info impl), `crates/forge_domain/src/context.rs` (token tracking).

### Phase 4: Info Panel Visual Improvements

- [ ] **4.1** Add a thin horizontal separator line between sections in `Info::fmt`. Currently sections run together with just a blank line. Add a `─────` separator (using `─` box-drawing characters) the full width of the longest key in the preceding section for alignment.

  Rationale: Sections like "AGENT", "TOKEN USAGE", "ENVIRONMENT" currently blend visually. A separator improves scannability.

  Key files: `crates/forge_main/src/info.rs:508-543` (`Display` impl).

- [ ] **4.2** Colorize numeric values in Info sections. Numbers representing quantities (tokens, costs, sizes) should be formatted with accent colors: green for completion/output metrics, cyan for input/cache metrics, yellow for costs. Detect numeric values in `into_value()` or add explicit `add_styled_key_value` for numeric display.

  Rationale: Plain white text doesn't draw the eye to the most important values in a wall of key-value pairs.

  Key files: `crates/forge_main/src/info.rs:508-543` (Display), `crates/forge_main/src/info.rs:238-269` (`IntoInfoValue` trait).

- [ ] **4.3** Add visual diff indicators for file change metrics. When `Metrics` lists file changes with `+N`/`−M`, render them with colored `+` (green) and `−` (red) Unicode minus signs and bold numbers. Currently it's plain `0 −2 +1` text.

  Rationale: Diff-style coloring is universally understood and makes it immediately obvious which files had significant changes.

  Key files: `crates/forge_main/src/info.rs:440-464` (Metrics From impl).

- [ ] **4.4** Add percentage bar for cache hit rate in usage display. When showing `Cached Tokens 5,000 [62%]`, render a tiny bar like `████░░░░ 62%` next to the percentage. Use Unicode block characters (`█░`) consistent with the `progress_bar.rs` style.

  Rationale: Cache hit percentage as a number is abstract; a small visual bar conveys it intuitively.

  Key files: `crates/forge_main/src/info.rs:472-481` (cached display logic).

### Phase 5: Prompt Polish and Micro-Interactions

- [ ] **5.1** Enhance the left prompt with a context-utilization indicator. Below the current dir/branch line, add a thin colored bar showing token usage as a proportion of the context window (e.g., `[████░░░░░░] 42%`). Only shown when context window is known and usage > 0.

  Rationale: The prompt is the most persistent UI element. Putting utilization there means users always know their context state without running `:usage`.

  Key files: `crates/forge_main/src/prompt.rs:53-103` (left prompt), `crates/forge_api` (context window info).

- [ ] **5.2** Add reasoning effort indicator to the right prompt (REPL, not just ZSH). The ZSH rprompt already shows reasoning effort (`crates/forge_main/src/zsh/rprompt.rs:83-154`). Bring the same to the REPL's `ForgePrompt::render_prompt_right()`.

  Rationale: Reasoning effort is important context that's only visible in ZSH — users in bare terminals don't see it.

  Key files: `crates/forge_main/src/prompt.rs:105-176`, `crates/forge_main/src/zsh/rprompt.rs` (reference implementation).

- [ ] **5.3** Improve the banner with adaptive tips and current model/agent info. Instead of static tips, show: (a) the currently configured agent and model, (b) a rotating contextual tip (e.g., if token usage is high, show "Use :compact to reduce context"), (c) version check result (if update available).

  Rationale: The banner is the first thing users see every session — it should be maximally informative.

  Key files: `crates/forge_main/src/banner.rs:56-114`, `crates/forge_main/src/ui.rs:153-159` (`display_banner`).

- [ ] **5.4** Add subtle transition markers between response phases. When a tool call starts, output a dim `┌ Running {tool_name}` header, and when it ends, output `└ Done` with timing info. This gives users a clear visual boundary for each tool invocation instead of just a bullet line.

  Rationale: Tool execution boundaries are currently invisible — the user sees a title line, then raw output, with no clear start/end framing.

  Key files: `crates/forge_main/src/ui.rs:3912-3934` (ToolCallStart), `crates/forge_main/src/ui.rs:3936-3953` (ToolCallEnd), `crates/forge_domain/src/chat_response.rs` (TitleFormat for tool calls).

### Phase 6: Accessibility and Consistency

- [ ] **6.1** Implement a `NO_COLOR` / `NO_NERD_FONT` graceful degradation pass. When `NO_COLOR=1` is set, strip all ANSI escapes. When nerd fonts are unavailable (detected at startup or via env var `NERD_FONT=0`), fall back to ASCII alternatives for all icons (`▶`, `✓`, `✗`, `[+]`, `[-]`, etc.). The ZSH rprompt already supports `use_nerd_font` — extend this to all UI components systematically.

  Rationale: ForgeCode should work well in basic terminals without font support.

  Key files: Global pass across `prompt.rs`, `title_display.rs`, `info.rs`, `banner.rs`, `todo_fmt.rs`, `display_constants.rs`.

- [ ] **6.2** Standardize timestamp format across all status lines. Currently `title_display.rs` uses `[HH:MM:SS]` format. Make this configurable and default to a more compact `HH:MM` (no seconds) unless verbose mode is active, to reduce visual noise.

  Rationale: Seconds-level timestamps clutter the output for routine messages. Minutes are sufficient for most interactions.

  Key files: `crates/forge_main/src/title_display.rs:37-38` (timestamp format).

- [ ] **6.3** Audit and normalize all `Info` key names to consistent Title Case. Currently some keys are Title Case ("Input Tokens") while others are lowercase in porcelain mode. Ensure the Display impl always uses Title Case for human output while maintaining exact headers for porcelain.

  Rationale: Inconsistent key casing (`logged in` vs `Input Tokens`) looks unprofessional.

  Key files: `crates/forge_main/src/ui.rs:1222-1228` (provider listing), `crates/forge_main/src/info.rs` (all Info constructions).

---

## Verification Criteria

- [ ] **Token delta display**: After sending a message, the right prompt shows an upward arrow with the token delta; after `:compact`, the delta shows a downward arrow.
- [ ] **Phase-aware spinner**: Spinner text changes deterministically per phase (Thinking → tool name → retry); no random word cycling.
- [ ] **Tool counter**: During multi-tool turns, spinner shows `Tool 2/5` or similar enumeration.
- [ ] **Category-distinct icons**: Each `Category` variant renders a distinct glyph (not all `●`).
- [ ] **Completion summary**: After `TaskComplete`, a single line summary appears with duration, tool count, and token delta.
- [ ] **Info separators**: Sections in `:info`, `:usage`, etc. are visually separated by horizontal lines.
- [ ] **Colored numbers**: Token counts, costs, and file diff numbers render in distinct accent colors.
- [ ] **Cache bar**: Tiny `████░░░░ 62%` bar appears next to cache percentage.
- [ ] **Context utilization bar**: Left prompt shows a proportional usage bar when tokens are active.
- [ ] **Reasoning effort in REPL**: Right prompt shows effort level (MED/HIGH/LOW) in the terminal, mirroring ZSH.
- [ ] **NO_COLOR compliance**: Setting `NO_COLOR=1` produces clean ANSI-free output for all components.
- [ ] **All existing tests pass**: `cargo insta test --accept` across all crates.

---

## Potential Risks and Mitigations

1. **Terminal compatibility with Unicode glyphs**
   Mitigation: All new glyphs must have ASCII fallbacks behind the `use_nerd_font` / `NO_COLOR` flag. Test in minimum-viable terminals (plain Linux VT, Windows cmd).

2. **Spinner message flicker on rapid phase transitions**
   Mitigation: Add a 100ms debounce on `set_message()` calls to prevent flashing between rapid tool calls. Coalesce sequential tool names if they come within the debounce window.

3. **Right prompt width overflow on narrow terminals**
   Mitigation: The delta token display (`▲1.2k`) should be conditionally hidden when terminal width is below a threshold (similar to `WIDE_TERMINAL_THRESHOLD` in `rprompt.rs:95`). The context bar on the left prompt should also collapse gracefully.

4. **Token delta tracking across compaction**
   Mitigation: Store the pre-compaction token count in `UIState` so the delta correctly shows a downward trend after `:compact` rather than resetting to zero.

5. **Performance impact of formatting token counts on every prompt render**
   Mitigation: Cache the formatted string in `ForgePrompt` and only recalculate when `usage` changes. The right prompt render is called on every keypress in some terminal configurations.

6. **Breaking porcelain (machine-readable) output**
   Mitigation: All visual enhancements apply only to human-readable (non-porcelain) output. Porcelain mode continues to output plain, parseable text. Gate all new formatting behind `if !porcelain`.

---

## Alternative Approaches

1. **Use a TUI framework (e.g., ratatui)**: Instead of line-by-line output with ANSI codes, build a proper terminal UI with panels, progress bars, and layout management.
   - *Trade-off*: Massive refactor, harder to maintain, may break pipe-ability and CI integration. The current streaming approach works well; this plan enhances it incrementally rather than replacing it.

2. **Web-based dashboard**: Add a `:dashboard` command that opens a browser view showing real-time token graphs, cost tracking, etc.
   - *Trade-off*: Requires significant new infrastructure (web server, asset bundling) and doesn't help during SSH sessions. Better as a future addition rather than core UX.

3. **Status bar via terminal title**: Use OSC 0/2 escape sequences to show status in the terminal tab/window title.
   - *Trade-off*: Not supported by all terminals, invisible when multiple panes are open. Could complement the in-terminal display but shouldn't replace it.