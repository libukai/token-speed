# Changelog

## Unreleased

- Fix: Codex forked threads (subagents) were attributed to their parent, so their own usage was dropped and legacy counters were reported instead. The cache is rebuilt once.
- DSH sessions run inside Palmier Pro are indexed alongside `~/.dsh`; source directory flags are repeatable.
- Dark / light themes: `--theme`, `TOKEN_SPEED_THEME`, `T`, or the `◐` toggle.
- Mouse: source, time-window and theme chips, table rows and chart buckets are clickable; clicks also close dialogs and keep a typed filter.
- Fix: clicks lagged or missed after pointer movement. Only press / release / wheel reporting is enabled, and events that change nothing no longer trigger a redraw each.
- DSH (DeepSeek Harness) source: reads `~/.dsh/sessions` zstd session logs, times each model step, and adds a fourth source to the TUI and reports.

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
