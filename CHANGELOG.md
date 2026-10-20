# Changelog

## Unreleased

The cache is rebuilt once.

- The DSH source is now **DeepSeek** (`--provider deepseek`, `--deepseek-dir`; `dsh` and `--dsh-dir` still work) and counts only calls made straight to DeepSeek's own API. Calls DSH routed through another provider, such as the Magpie gateway, are skipped.
- WorkBuddy source: reads the per-request `qua.end` lines in `~/.workbuddy/logs` (`--workbuddy-dir`, `WORKBUDDY_HOME`) with the request's token counts and total time. It has no cache counters, so cache stays zero.
- Magpie ledger calls from the agent `dsh` are no longer filed under DeepSeek; they go to the hidden `other` source.

## [0.4.0](https://github.com/libukai/token-speed/releases/tag/v0.4.0) — 2026-10-18

First release of this fork. The cache is rebuilt once.

- Fix: Codex forked threads (subagents) were attributed to their parent, so their own usage was dropped and legacy counters were reported instead. The cache is rebuilt once.
- Source directory flags (`--codex-dir`, `--claude-dir`, `--dsh-dir`) are repeatable.
- Dark / light themes: `--theme`, `TOKEN_SPEED_THEME`, `T`, or the `◐` toggle.
- Mouse: source, time-window and theme chips, table rows and chart buckets are clickable; clicks also close dialogs and keep a typed filter.
- Fix: clicks lagged or missed after pointer movement. Only press / release / wheel reporting is enabled, and events that change nothing no longer trigger a redraw each.
- DSH (DeepSeek Harness) source: reads `~/.dsh/sessions` zstd session logs, times each model step, and adds a fourth source to the TUI and reports.
- Magpie gateway ledger (`~/.config/magpie/usage.jsonl`, `--magpie-dir`): calls are filed under the agent that made them, and the native log wins where both exist. Calls from agents without a parser go to a hidden `other` source (`--provider other`).
- The live status moved from above the key legend to under the title.
- Fix: a gateway record could replace the agent's own record with the same id, and a stale native start could hide gateway-only calls.
- Fix: Magpie calls without a response id collapsed into one; Claude messages copied into a resumed session were counted twice.
- Fix: a cold index held the cache write lock throughout, so a second instance could fail with "database is locked". The cache now uses WAL and short write batches.
- Fix: a file vanishing mid-index failed the whole pass; it is now a warning.
- Fix: a day whose local midnight is skipped by DST could not be selected, and pressing `t` on it closed the TUI. Failed actions now show in the status line.
- `TZ` in POSIX form (`:/etc/localtime`, `JST-9`) falls back to the system zone; `--interval` must be at least 1 second.
- The selected request or model stays selected across refreshes.
- CSV cells starting like a spreadsheet formula are prefixed with `'`.
- npm package renamed to `@libukai/token-speed`.

## [0.3.1](https://github.com/william1010121/token-speed/releases/tag/v0.3.1) — 2026-10-08

- npm packages with checksum-verified native binaries and a lightweight launcher.
- Cargo Git installation instructions and npm version badges in both READMEs.
- Graceful Unix TUI shutdown on SIGINT, SIGTERM and SIGHUP, restoring terminal state.
- Bordered text reports with right-aligned numbers and weighted summary totals.
- `--table` and `--json` shortcuts; explicit `--format table` also bypasses the TUI.

## [0.3.0](https://github.com/william1010121/token-speed/releases/tag/v0.3.0) — 2026-10-08

First public release.

- Native Rust CLI and Ratatui dashboard with Overview, Timeline, Models and Requests views.
- Gap-aware split / overlay timelines with coverage indicators and a bucket inspector.
- Model rate distributions, P50/P90 and input-size scatter plots.
- Local Codex / Claude Code parsing, deduplication and explicit estimated-timing coverage.
- Incremental SQLite metadata cache with background refresh and concurrent-access handling.
- Provider, model, session, date and timezone filters; table / JSON / CSV exports.
- Keyboard navigation, mouse selection and compact layouts down to 60 × 20.
- Synthetic screenshots, documentation, GitHub CI and native release archives.
