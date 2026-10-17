# Working on token-speed

Rust CLI / Ratatui TUI for local Codex and Claude Code usage logs. Keep the runtime local and the measurement claims accurate. See `docs/metrics.md` before changing parsing or aggregation.

- `src/source.rs`: source registry (ids, homes, log roots, parser); add sources there first.
- `src/parse/`: one parser per source; `src/data.rs`: indexing, SQLite cache, metrics.
- `src/args.rs`: CLI; `src/report.rs`: table / JSON / CSV reports.
- `src/app.rs`: state, input handling, background indexing; `src/ui/`: rendering, one module per view, themes in `ui/theme.rs`.
- `src/tests.rs`: synthetic parser fixtures, interaction and layout checks.
- Run Cargo commands from the repo root. Rust 1.92+; SQLite is bundled. Apple targets use `/usr/bin/clang` via `.cargo/config.toml`.
- Validate meaningful changes with `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo fmt --check`.
- Keep screenshots and fixtures synthetic. Do not read personal logs for tests or commit source logs, caches, credentials or absolute personal paths.

## Code Review Rules

### Measurement integrity

Flag changes that double-count mirrored Codex usage, repeated Claude blocks, cache input or reasoning output; mix Claude branches when estimating starts; or include invalid intervals in weighted throughput. Untimed usage must remain in usage totals. Missing timing must remain a gap, not zero or interpolated speed. Request throughput must not be described as pure decode speed. Safe path: preserve these semantics and add a synthetic regression fixture when changing them.

### Local data boundary

Flag new network access, persistence of prompt / tool bodies, or exposure of private logs in artifacts and CI. Cache and export metadata intentionally include session IDs and source paths; that documented behavior is allowed. Safe path: index usage metadata only, and use synthetic examples for public artifacts.

### Interactive reliability

Flag changes that block input while indexing, leave raw mode / mouse capture enabled after errors, or panic on empty data, resized terminals or concurrent cache access. Split and overlay views must keep missing timing visible. Safe path: background indexing, terminal cleanup on every exit, bounded layout / selection handling and serialized cache writes.
