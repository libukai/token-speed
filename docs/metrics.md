# How token-speed measures throughput

The unit is a model response with usage metadata. A single user turn can contain several responses and tool round trips.

```text
request tok/s = output tokens / estimated request seconds
weighted tok/s = sum(output tokens with usable timing) / sum(usable request seconds)
```

Median, P50 and P90 describe individual valid request rates. Overlapping requests each contribute their own duration; the denominator is not total wall-clock time.

## Timing and exclusions

Codex uses a user / task start, a tool-result boundary or the previous usage event as the start. Claude follows `parentUuid` ancestry on the same branch to a user event (including tool results) or the last block of a previous assistant response. DSH records one model call per `step`: the start is that step's `step/start` event and the end is its `assistant/message`, so tool execution between steps is not included.

These intervals can include prefill, reasoning, network, retries, orchestration and residual waits. Logs usually cannot isolate first-token and last-token timestamps. This is estimated request throughput, not pure decode speed. Provider tokenizers, workload, context and reasoning differ: comparisons are observational, not controlled benchmarks.

Missing starts, nonpositive durations and intervals above `--max-gap` (default 300 seconds) are excluded from speed. Their usage still counts. Genuine long requests may also be excluded; adjust the threshold if appropriate. JSON / CSV expose timing quality as `estimated`, `missing` or `excluded`.

## Usage normalization and deduplication

- **Codex:** prefer `token_usage_record.usage`, deduplicate by session / response ID, and ignore other-thread mirrors in a parent log. A forked thread's log repeats its parent's `session_meta` after its own; the first one identifies the file's session. When a file contains modern records, legacy `token_count` is ignored. Legacy-only files use cumulative differences, skip duplicate snapshots and use `last_token_usage` after counter resets. The first legacy snapshot contributes only last usage; earlier requests are not invented.
- **DSH:** sessions are zstd-compressed JSONL appended one frame per write; a session upgraded in place (`session.jsonl` and `session.v4.jsonl`) is counted once; a frame cut off mid-write is skipped as an incomplete tail. Usage comes from `assistant/message` events, deduplicated by message ID with maximum counters. A message without a matching `step/start` keeps its usage but has no timing.
- **Magpie:** `usage.jsonl` is the gateway's ledger: one line per model call from any agent behind it. Each call is filed under its agent's source (`codex`, `claude`, `dsh`) and the rest under `other` (the agent's name leads the session, e.g. `pi/<session>`), which is hidden: no tab, key, or share of the totals unless `--provider other` names it. So `--provider dsh` includes the DSH calls only the gateway saw. Ledger and agent ids are not relied on to match, so a ledger call is dropped when it completes within 10 seconds of a request span in that agent's own log (same session where the ledger names the agent's session, any session for DSH); a span runs from the request's start, when that start is usable under `--max-gap`, to its end. Calls outside those spans stay, and a ledger record never replaces an agent record that happens to share its id. A ledger line without a response id is its own call. The agent's own log therefore wins where both exist, and an agent with no log, or a stretch where its log is missing, is counted from the ledger. Calls without usage (rejected, cancelled) are skipped. Timing is the gateway's: it starts when the request is received and ends when the reply finishes, so it also includes the network and gateway overhead the agent's own timestamps may not (`timing_source` is `gateway`). Input is the gateway's uncached input plus cache reads and writes. The ledger carries no reasoning split for most calls. Only usage metadata is read; the other files in the Magpie directory (logins, plugin credentials) are not.
- **Claude Code:** merge assistant blocks by message ID, take maximum usage counters and the last block timestamp, then deduplicate across files by message ID alone: a resumed session copies earlier messages into its new file under a new session ID. Missing parent records retain usage but may prevent a timing estimate.
- **Input:** Codex input already includes cache. Claude and DSH input are normalized by adding cache-read and cache-write tokens. Cached tokens are a subset of input.
- **Output:** reasoning is already included in output and must not be added again. Separate reasoning counters are kept when available.

Mixed-schema Codex files can omit usage represented only by legacy events. Changed upstream schemas may require parser updates; malformed lines produce a warning count.

## Time ranges and chart buckets

Requests are attributed to their completion timestamp. `--until YYYY-MM-DD` includes that whole date; a full ISO timestamp is an exclusive upper bound. IANA timezone handling respects historical DST. A calendar date starts at its first instant, which is after the gap where DST skips midnight. Ambiguous or nonexistent local clock times need an explicit UTC offset. `--timezone` must be an IANA name; `TZ` is used when it is one and ignored otherwise (for example `:/etc/localtime`).

Timeline bins are hourly for today, six-hourly for ranges up to eight days, daily up to 32 days, and approximately 48 bins for longer ranges. Daily chart bins are fixed 24-hour spans, so they can differ from local calendar days at DST transitions. Labels use the chosen timezone.

A curve gap is not zero throughput. `░` means no requests; `!` means requests without usable timing. Coverage strips indicate output volume normalized separately within each source. Overview output bars use a shared scale.

## Cache and privacy

The SQLite cache uses path, size and nanosecond mtime to find changes. A changed file is reparsed; deleted files are excluded from reports. The cache runs in WAL mode: lookups never wait for a writer, and newly parsed files are stored in short serialized write batches with a busy timeout, so concurrent instances do not lock each other out. A file that vanishes during indexing is skipped with a warning. An incomplete tail is retried when the file changes.

Only usage metadata is indexed, including source paths and session IDs. Prompts and tool bodies are not persisted. No network service is contacted by the application. Exported reports and the cache can still reveal identifying metadata, so share synthetic or redacted examples.

There is no stream-measurement mode or extra model call in this release.
