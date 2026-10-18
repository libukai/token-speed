<div align="center">

# token-speed

**Your coding agents, at a glance.**

A terminal observatory for Codex, Claude Code and DSH (DeepSeek Harness), plus any agent behind a Magpie gateway.<br>
Four views. Local logs. One native Rust binary.<br>
<sub>A fork of [william1010121/token-speed](https://github.com/william1010121/token-speed) with DSH and Magpie gateway sources.</sub>

[![CI](https://github.com/libukai/token-speed/actions/workflows/ci.yml/badge.svg)](https://github.com/libukai/token-speed/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-1.92%2B-dea584?logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-74b9ff)](LICENSE)

[Quick start](#quick-start) · [Views](#four-ways-to-read-your-work) · [CLI](#scriptable-by-design) · [Metrics](docs/metrics.md) · [繁體中文](README.zh-TW.md)

<img src="docs/preview.png" alt="token-speed Overview: provider throughput cards, activity map, recent requests and model comparison" width="1100">

<sub>All screenshots use synthetic demo data.</sub>

</div>

## Why token-speed?

You already have usage logs. See when your agents were active, which models were faster in your workload, and which requests need a closer look — without leaving the terminal.

- **Four connected views:** overview, timeline, models and individual requests. Keep your filters as you move.
- **Honest gaps:** missing activity and missing timing have distinct indicators. Curves never invent measurements between them.
- **Useful comparisons:** weighted throughput, P50/P90, speed distributions and context-size scatter plots.
- **Local by default:** no API keys, model calls or telemetry. An incremental SQLite index keeps subsequent reads fast.
- **Works in scripts:** table, JSON and CSV reports alongside the interactive dashboard.

> [!IMPORTANT]
> **tok/s means estimated request throughput**, calculated from output tokens and log timestamps. It includes reasoning, prefill, network delays and residual waits. It is not pure streaming decode speed or a controlled comparison between providers. Timing coverage stays visible; untimed usage still counts. [Read the measurement details →](docs/metrics.md)

## Quick start

This fork is not published to npm or crates.io, and it has no prebuilt release yet: the `token-speed` package on npm is the [upstream](https://github.com/william1010121/token-speed) project, without the DSH and Magpie sources. Build this fork from source:

```bash
cargo install --git https://github.com/libukai/token-speed --locked
token-speed --today
```

Requires **Rust 1.92+** and a C compiler. SQLite is bundled. Cargo installs to `~/.cargo/bin` by default; ensure it is on your `PATH`. On macOS, if a custom compiler wrapper causes SDK/linker errors, set `CC=/usr/bin/clang` and `CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/clang` (Intel: `CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER`).

If you only need Codex and Claude Code, the upstream npm package and prebuilt binaries work: `npm install -g token-speed`.

### Build from source

Requires **Rust 1.92+** and a C compiler. SQLite is bundled.

```bash
git clone https://github.com/libukai/token-speed.git
cd token-speed
./install.sh
token-speed --today
```

The installer builds with the committed lockfile and writes to `~/.local/bin`. Set `TOKEN_SPEED_INSTALL_DIR` to change the destination. If needed, add `export PATH="$HOME/.local/bin:$PATH"` to your shell profile.

Use a truecolor terminal. **120 × 40** is recommended; **80 × 24** uses a compact layout. The minimum is **60 × 20**.

## Four ways to read your work

| View | What you see |
| --- | --- |
| **Overview** | Large throughput cards, activity map, latest requests and model rankings. |
| **Timeline** | Split or overlaid provider curves, coverage strips and a movable bucket inspector. |
| **Models** | A comparison table, P50/P90, rate histogram and input-size / throughput scatter plot. |
| **Requests** | A scrollable request ledger with token counts, timing quality and session details. |

<details>
<summary><strong>Timeline</strong> · See exactly why a curve has a gap</summary>

![Timeline with independent provider plots and selected time bucket](docs/preview-timeline.png)

Press `g` for split / overlay. Use `←` / `→` to inspect a bucket. `░` means no requests; `!` means usage without usable timing. Colored blocks indicate output volume, normalized within each provider. The inspector explains the selected bucket.

</details>

<details>
<summary><strong>Models</strong> · Compare distributions, not just averages</summary>

![Models with comparison table, throughput histogram and context scatter plot](docs/preview-models.png)

Select a model to inspect its P50/P90, request-rate distribution and context-size relationship.

</details>

<details>
<summary><strong>Requests</strong> · Follow a number back to a request</summary>

![Requests with scrollable records and metadata sidebar](docs/preview-requests.png)

Inspect individual responses and the interval used to estimate throughput. Press `Enter` for expanded details.

</details>

```bash
token-speed                         # Last seven days, Overview
token-speed --today --view timeline
token-speed --days 30 --view models
token-speed --provider claude --model opus --view requests
```

## Keyboard-first, mouse-friendly

| Key | Action |
| --- | --- |
| `Tab` / `Shift-Tab`, `F1`–`F4` | Switch views |
| Mouse | Click tabs, sources, time windows, table rows (again for details), chart buckets and `◐` theme; wheel scrolls |
| `1` / `2` / `3` / `4` | All / Codex / Claude / DSH (shown once DSH logs exist) |
| `t` / `w` / `m` / `a`, `d` | Today / 7 days / 30 days / all; cycle range |
| `T` | Toggle dark / light theme |
| `/`, `Esc` | Filter by model or session; close / clear |
| `↑` / `↓`, `j` / `k`, mouse wheel | Select rows; `Home` / `End` jump to ends |
| `←` / `→`, `g` | Timeline bucket / split or overlay |
| `Enter` | Request details |
| `Space`, `r` | Pause automatic updates / refresh |
| `?`, `q` / `Ctrl-C` | Help / quit |

Updates run in the background every five seconds. `--interval` adjusts this. The TUI retains at least 200 recent requests; `--limit` can increase that window.

## Scriptable by design

```bash
# Plain text table with borders and a weighted total row
token-speed --table --today

# JSON summary (stdout contains only JSON)
token-speed --json --days 7 > speed.json

# Hourly throughput today
token-speed summary --today --group hour

# Model comparison over a longer period
token-speed summary --days 30 --group model --provider codex

# Recent responses, or machine-readable exports
token-speed recent --today --limit 20
token-speed recent --json --today --limit 20 > requests.json
token-speed summary --days 7 --format json > speed.json
token-speed recent --days 30 --format csv > requests.csv

# Inclusive dates in a specific timezone
token-speed --since 2026-10-01 --until 2026-10-08 --timezone Asia/Taipei
```

`--table`, `--json`, or an explicit `--format` produce a report even in an interactive terminal. With no options, the dashboard remains the default. `summary` and `recent` always produce reports; with no interactive terminal, the default command produces a report too. `watch` requires an interactive terminal and uses the dashboard. Run `token-speed --help` for every option.

Text tables have borders, right-aligned numeric columns and, for summaries, a total row computed from all matching requests. Missing or excluded timing appears as `—`; those requests still contribute to usage totals. JSON keeps the existing `metric`, `max_gap_seconds`, `timezone`, `metadata` and `data` fields. Missing speeds are `null`; recent requests also include `timing_status` (`estimated`, `missing`, or `excluded`).

| Option | Purpose |
| --- | --- |
| `--provider all\|codex\|claude\|dsh\|magpie` | Choose source |
| `--model`, `--session` | Substring filters |
| `--today`, `--days N`, `--all-time`, `--since`, `--until` | Time range |
| `--timezone` | IANA timezone; defaults to `TZ` or local timezone |
| `--view` | Initial dashboard view |
| `--theme dark\|light` | Dashboard colors (`TOKEN_SPEED_THEME`); `T` toggles while running |
| `--table`, `--json` | One-shot text table or JSON report; bypass the TUI |
| `--group hour\|day\|model`, `--format table\|json\|csv` | Report grouping and format |
| `--max-gap SECONDS` | Maximum usable timing interval; default 300 |
| `--codex-dir`, `--claude-dir`, `--dsh-dir`, `--magpie-dir`, `--cache` | Override source / index locations |
| `--refresh` | Rebuild cached entries |

## Your logs stay on your machine

| Source | Default location | Override |
| --- | --- | --- |
| Codex | `~/.codex/{sessions,archived_sessions}` | `CODEX_HOME` or `--codex-dir` |
| Claude Code | `~/.claude/projects` | `CLAUDE_CONFIG_DIR` or `--claude-dir` |
| DSH (DeepSeek Harness) | `~/.dsh/sessions` (`*.jsonl.zstd`) | `DSH_HOME` or `--dsh-dir` (repeatable) |
| Magpie gateway (`agent` per call) | `~/.config/magpie/usage.jsonl` | `MAGPIE_HOME` or `--magpie-dir` (repeatable) |
| SQLite index | `~/.cache/token-speed/index.sqlite3` | `XDG_CACHE_HOME` or `--cache` |

The parser includes subagent logs and deduplicates supported usage records. The cache stores token counts, timestamps, model names, session identifiers and source paths. It does **not** store prompts or tool-output bodies. Reports can contain session identifiers and file paths; redact them before sharing.

The first run indexes your history. Later runs scan file metadata and reparse changed files. An incomplete JSONL tail is skipped and retried when the file changes. See [metrics and parser behavior](docs/metrics.md) for schema limitations.

## Contributing

Run `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo fmt --check`. Use synthetic fixtures; never commit personal agent logs. [Contributing guide](CONTRIBUTING.md) · [Codex review setup](docs/codex-review.md) · [Changelog](CHANGELOG.md).

Maintainers: [npm packaging and publishing](docs/publishing.md).

## Upstream and credits

This is a fork of **[william1010121/token-speed](https://github.com/william1010121/token-speed)** (MIT), which provides the Codex and Claude Code parsers, the four views, the SQLite index and the npm / Cargo packaging. This fork adds:

- **DSH** (DeepSeek Harness) as a source, with per-step timing.
- The **Magpie** gateway ledger as a source, so agents behind it are counted and gaps in their own logs are filled.
- A source registry, dark / light themes and mouse support, Codex / Claude speed mode, and the live status line under the title.

Changes that suit the upstream project are welcome there too. The original copyright notice is kept in [LICENSE](LICENSE).

Built with [Ratatui](https://ratatui.rs/) and [Crossterm](https://github.com/crossterm-rs/crossterm). Independent community project; not affiliated with OpenAI or Anthropic.

[MIT license](LICENSE).
