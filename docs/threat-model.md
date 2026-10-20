# token-speed threat model

## Assets and data flow

The Rust binary reads Codex, Claude Code, DeepSeek (DSH), WorkBuddy and Magpie usage logs chosen through CLI arguments / environment variables. It extracts usage metadata into a local SQLite index and displays or exports reports. Prompts and tool outputs are present in source logs but are not persisted by the index. Session identifiers, source paths, model names, timestamps and counts are intentionally retained and can identify activity.

## Trust boundaries

- Treat log contents, source paths and model / session strings as untrusted input. Parsing must not execute log content or make network requests. Terminal output must not interpret injected escape sequences from metadata.
- The local user controls input roots, cache path and export destinations. Writing the chosen cache is expected; altering source logs or unrelated files is not.
- Multiple instances can access the same cache. Failures must preserve recoverable data and restore terminal state. A local same-user attacker already controls the user's files; this is not a multi-user service or authentication boundary.
- Public screenshots, fixtures, CI logs and release assets must contain synthetic data, not personal source logs, credentials or cache files.
- GitHub Actions executes repository build scripts and dependencies. Pull-request workflows use read-only permissions. Only the release publish job receives repository write permission; third-party actions are pinned to full commits.

## Review priorities

Flag prompt / tool-body persistence, outbound telemetry or arbitrary execution introduced through parsing. Check unbounded malformed-input resource use, unsafe filesystem changes, terminal escape injection and accidental disclosure through error messages or public artifacts. For cache changes, consider corruption, concurrency and schema migration failures. Token accounting defects are correctness issues covered by `AGENTS.md`; do not invent network / server attack surfaces the application does not have.

Release checksums detect damaged archives but are not independent publisher authentication. The program depends on the user's filesystem permissions and trusted installation source.
