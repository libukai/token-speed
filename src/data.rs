use crate::args::{Args, Group, Provider};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
    time::UNIX_EPOCH,
};
use walkdir::WalkDir;

const CACHE_VERSION: i64 = 3;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub provider: String,
    pub session: String,
    pub id: String,
    pub model: String,
    pub timestamp: f64,
    pub start: Option<f64>,
    pub input_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub timing_source: String,
}
impl Record {
    pub fn duration(&self, max_gap: f64) -> Option<f64> {
        let d = self.timestamp - self.start?;
        (d > 0.0 && d <= max_gap && self.output_tokens > 0).then_some(d)
    }
    pub fn rate(&self, max_gap: f64) -> Option<f64> {
        Some(self.output_tokens as f64 / self.duration(max_gap)?)
    }
    pub fn key(&self) -> (String, String, String) {
        (self.provider.clone(), self.session.clone(), self.id.clone())
    }
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Metadata {
    pub parsed_files: usize,
    pub cached_files: usize,
    pub warnings: Vec<String>,
}

pub fn stamp(v: &Value) -> Option<f64> {
    if let Some(n) = v.as_f64() {
        return Some(if n > 100_000_000_000.0 { n / 1000.0 } else { n });
    }
    let s = v.as_str()?;
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Some(d.timestamp_millis() as f64 / 1000.0);
    }
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
        .ok()
        .map(|d| d.and_utc().timestamp_millis() as f64 / 1000.0)
}
fn text(v: &Value) -> Option<&str> {
    v.as_str().filter(|s| !s.is_empty())
}
fn n(v: &Value, k: &str) -> u64 {
    v[k].as_u64()
        .unwrap_or_else(|| v[k].as_f64().unwrap_or(0.0).max(0.0) as u64)
}
fn make(
    provider: &str,
    session: &str,
    id: &str,
    model: &str,
    end: f64,
    start: Option<f64>,
    u: &Value,
) -> Record {
    let cached = n(
        u,
        if provider == "codex" {
            "cached_input_tokens"
        } else {
            "cache_read_input_tokens"
        },
    );
    let written = n(
        u,
        if provider == "codex" {
            "cache_write_input_tokens"
        } else {
            "cache_creation_input_tokens"
        },
    );
    Record {
        provider: provider.into(),
        session: session.into(),
        id: id.into(),
        model: model.into(),
        timestamp: end,
        start,
        input_tokens: n(u, "input_tokens")
            + if provider == "claude" {
                cached + written
            } else {
                0
            },
        cached_tokens: cached,
        cache_write_tokens: written,
        output_tokens: n(u, "output_tokens"),
        reasoning_tokens: if provider == "codex" {
            n(u, "reasoning_output_tokens")
        } else {
            n(&u["output_tokens_details"], "thinking_tokens")
        },
        timing_source: if provider == "codex" {
            "request-boundary"
        } else {
            "parent-boundary"
        }
        .into(),
    }
}
#[derive(Clone)]
struct Node {
    kind: String,
    time: Option<f64>,
    parent: Option<String>,
    message_key: (String, String),
}

/// Decode concatenated zstd frames. DSH appends one frame per write, so a
/// frame cut off mid-write is dropped whole and reported as an incomplete tail.
fn inflate(mut input: &[u8]) -> (Vec<u8>, bool) {
    use ruzstd::decoding::{BlockDecodingStrategy, FrameDecoder, errors};
    let mut out = Vec::new();
    let mut decoder = FrameDecoder::new();
    while !input.is_empty() {
        let frame_start = out.len();
        match decoder.init(&mut input) {
            Ok(()) => {}
            Err(errors::FrameDecoderError::ReadFrameHeaderError(
                errors::ReadFrameHeaderError::SkipFrame { length, .. },
            )) => match input.get(length as usize..) {
                Some(rest) => {
                    input = rest;
                    continue;
                }
                None => return (out, true),
            },
            Err(_) => return (out, true),
        }
        while !decoder.is_finished() {
            let before = input.len();
            let ok = decoder
                .decode_blocks(&mut input, BlockDecodingStrategy::UptoBytes(1 << 20))
                .is_ok()
                && decoder.collect_to_writer(&mut out).is_ok();
            if !ok || (input.len() == before && !decoder.is_finished()) {
                out.truncate(frame_start);
                return (out, true);
            }
        }
        if decoder.collect_to_writer(&mut out).is_err() {
            out.truncate(frame_start);
            return (out, true);
        }
    }
    (out, false)
}

pub fn is_log(path: &Path, provider: &str) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    name.ends_with(".jsonl")
        || provider == "dsh" && (name.ends_with(".jsonl.zstd") || name.ends_with(".jsonl.zst"))
}

/// DSH (DeepSeek Harness) session events. Each model call is bracketed by
/// `step/start` and an `assistant/message` carrying usage for that step.
fn parse_dsh(path: &Path) -> Result<(Vec<Record>, usize)> {
    let mut raw = Vec::new();
    File::open(path)?.read_to_end(&mut raw)?;
    let compressed = path.extension().is_some_and(|x| x == "zstd" || x == "zst");
    let (bytes, truncated) = if compressed {
        inflate(&raw)
    } else {
        (raw, false)
    };
    let mut session = path
        .parent()
        .and_then(|p| p.file_name())
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut malformed = usize::from(truncated);
    let mut steps: HashMap<(u64, u64), f64> = HashMap::new();
    let mut messages: HashMap<String, Record> = HashMap::new();
    for line in bytes.split(|b| *b == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let d: Value = match serde_json::from_slice::<Value>(line) {
            Ok(v) if v.is_object() => v,
            _ => {
                malformed += 1;
                continue;
            }
        };
        let p = &d["data"];
        let step = (n(p, "turn"), n(p, "step"));
        match text(&d["type"]).unwrap_or("") {
            "session" => session = text(&d["id"]).unwrap_or(&session).to_owned(),
            "step/start" => {
                if let Some(t) = stamp(&d["time"]) {
                    steps.insert(step, t);
                }
            }
            "assistant/message" if p["usage"].is_object() => {
                let Some(end) = stamp(&d["time"]) else {
                    continue;
                };
                let u = &p["usage"];
                let cached = n(u, "cacheReadTokens");
                let written = n(u, "cacheWriteTokens");
                let id = text(&p["message"]["id"])
                    .map(String::from)
                    .unwrap_or_else(|| format!("turn-{}-step-{}", step.0, step.1));
                let r = Record {
                    provider: "dsh".into(),
                    session: session.clone(),
                    id: id.clone(),
                    model: text(&p["message"]["source"]["model"])
                        .unwrap_or("unknown")
                        .into(),
                    timestamp: end,
                    start: steps.get(&step).copied(),
                    // DSH reports cache reads separately from uncached input.
                    input_tokens: n(u, "inputTokens") + cached + written,
                    cached_tokens: cached,
                    cache_write_tokens: written,
                    // Reasoning is already part of output.
                    output_tokens: n(u, "outputTokens"),
                    reasoning_tokens: n(u, "reasoningTokens"),
                    timing_source: "step-boundary".into(),
                };
                if let Some(prev) = messages.get_mut(&id) {
                    prev.timestamp = prev.timestamp.max(r.timestamp);
                    prev.input_tokens = prev.input_tokens.max(r.input_tokens);
                    prev.output_tokens = prev.output_tokens.max(r.output_tokens);
                    prev.cached_tokens = prev.cached_tokens.max(r.cached_tokens);
                    prev.cache_write_tokens = prev.cache_write_tokens.max(r.cache_write_tokens);
                    prev.reasoning_tokens = prev.reasoning_tokens.max(r.reasoning_tokens);
                } else {
                    messages.insert(id, r);
                }
            }
            _ => {}
        }
    }
    let mut rows: Vec<_> = messages.into_values().collect();
    rows.sort_by(|a, b| a.timestamp.total_cmp(&b.timestamp));
    Ok((rows, malformed))
}

pub fn parse_file(path: &Path, provider: &str) -> Result<(Vec<Record>, usize)> {
    if provider == "dsh" {
        return parse_dsh(path);
    }
    let mut session = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut model = "unknown".to_owned();
    let mut anchor = None;
    let mut cumulative: Option<Value> = None;
    let (mut modern, mut legacy) = (Vec::new(), Vec::new());
    let mut malformed = 0;
    let mut messages: HashMap<(String, String), Record> = HashMap::new();
    let mut parents: HashMap<(String, String), Option<String>> = HashMap::new();
    let mut nodes: HashMap<String, Node> = HashMap::new();
    let mut stream = BufReader::with_capacity(256 * 1024, File::open(path)?);
    let mut line = Vec::new();
    let mut index = 0;
    loop {
        line.clear();
        if stream.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        index += 1;
        let d: Value = match serde_json::from_slice::<Value>(&line) {
            Ok(v) if v.is_object() => v,
            _ => {
                malformed += 1;
                continue;
            }
        };
        let t = stamp(&d["timestamp"]);
        let typ = text(&d["type"]).unwrap_or("");
        if provider == "codex" {
            let p = &d["payload"];
            let sub = text(&p["type"]).unwrap_or("");
            match typ {
                "session_meta" => {
                    session = text(&p["id"])
                        .or_else(|| text(&p["session_id"]))
                        .unwrap_or(&session)
                        .to_owned();
                }
                "turn_context" => {
                    model = text(&p["model"]).unwrap_or(&model).to_owned();
                }
                "event_msg" if sub == "task_started" => {
                    anchor = stamp(&p["started_at"]).or(t);
                }
                "response_item"
                    if matches!(sub, "function_call_output" | "custom_tool_call_output")
                        || (sub == "message" && p["role"] == "user") =>
                {
                    anchor = t;
                }
                "token_usage_record" if t.is_some() => {
                    if text(&p["thread_id"]).is_some_and(|id| id != session) {
                        continue;
                    }
                    let fallback = format!("line-{index}");
                    modern.push(make(
                        provider,
                        &session,
                        text(&p["response_id"]).unwrap_or(&fallback),
                        &model,
                        t.unwrap(),
                        anchor,
                        &p["usage"],
                    ));
                    anchor = t;
                }
                "event_msg" if sub == "token_count" && t.is_some() => {
                    let info = &p["info"];
                    let total = &info["total_token_usage"];
                    let last = &info["last_token_usage"];
                    if !total.is_object() || !last.is_object() || cumulative.as_ref() == Some(total)
                    {
                        continue;
                    }
                    let usage = if let Some(prev) = cumulative
                        .as_ref()
                        .filter(|prev| n(total, "output_tokens") >= n(prev, "output_tokens"))
                    {
                        Value::Object(
                            total
                                .as_object()
                                .unwrap()
                                .keys()
                                .map(|k| {
                                    (
                                        k.clone(),
                                        Value::from(n(total, k).saturating_sub(n(prev, k))),
                                    )
                                })
                                .collect(),
                        )
                    } else {
                        last.clone()
                    };
                    cumulative = Some(total.clone());
                    legacy.push(make(
                        provider,
                        &session,
                        &format!("line-{index}"),
                        &model,
                        t.unwrap(),
                        anchor,
                        &usage,
                    ));
                    if modern.is_empty() {
                        anchor = t;
                    }
                }
                _ => {}
            }
        } else {
            let sid = text(&d["sessionId"])
                .or_else(|| text(&d["session_id"]))
                .unwrap_or(&session)
                .to_owned();
            let mid = text(&d["message"]["id"]).unwrap_or("").to_owned();
            if let Some(uuid) = text(&d["uuid"]) {
                nodes.insert(
                    uuid.to_owned(),
                    Node {
                        kind: typ.to_owned(),
                        time: t,
                        parent: text(&d["parentUuid"]).map(String::from),
                        message_key: (sid.clone(), mid.clone()),
                    },
                );
            }
            if typ != "assistant"
                || t.is_none()
                || !d["message"]["usage"].is_object()
                || d["message"]["model"] == "<synthetic>"
            {
                continue;
            }
            let fallback = format!("line-{index}");
            let mid = text(&d["message"]["id"])
                .or_else(|| text(&d["requestId"]))
                .or_else(|| text(&d["uuid"]))
                .unwrap_or(&fallback);
            let key = (sid.clone(), mid.to_owned());
            let r = make(
                provider,
                &sid,
                mid,
                text(&d["message"]["model"]).unwrap_or("unknown"),
                t.unwrap(),
                None,
                &d["message"]["usage"],
            );
            if let Some(prev) = messages.get_mut(&key) {
                prev.timestamp = prev.timestamp.max(r.timestamp);
                prev.input_tokens = prev.input_tokens.max(r.input_tokens);
                prev.output_tokens = prev.output_tokens.max(r.output_tokens);
                prev.cached_tokens = prev.cached_tokens.max(r.cached_tokens);
                prev.cache_write_tokens = prev.cache_write_tokens.max(r.cache_write_tokens);
                prev.reasoning_tokens = prev.reasoning_tokens.max(r.reasoning_tokens);
            } else {
                parents.insert(key.clone(), text(&d["parentUuid"]).map(String::from));
                messages.insert(key, r);
            }
        }
    }
    let rows = if provider == "codex" {
        if modern.is_empty() { legacy } else { modern }
    } else {
        let mut rows = Vec::new();
        for (key, r) in &messages {
            let mut r = r.clone();
            let mut parent = parents.get(key).cloned().flatten();
            let mut seen = HashSet::new();
            while let Some(id) = parent {
                if !seen.insert(id.clone()) {
                    break;
                }
                let Some(node) = nodes.get(&id) else {
                    break;
                };
                if node.kind == "user" {
                    r.start = node.time;
                    break;
                }
                if node.kind == "assistant" && &node.message_key != key {
                    r.start = messages
                        .get(&node.message_key)
                        .map(|p| p.timestamp)
                        .or(node.time);
                    break;
                }
                parent = node.parent.clone();
            }
            rows.push(r);
        }
        rows
    };
    let mut unique = HashMap::new();
    for r in rows {
        unique.insert(r.key(), r);
    }
    let mut rows: Vec<_> = unique.into_values().collect();
    rows.sort_by(|a, b| a.timestamp.total_cmp(&b.timestamp));
    Ok((rows, malformed))
}

pub fn collect(
    args: &Args,
    mut progress: impl FnMut(usize, usize),
) -> Result<(Vec<Record>, Metadata)> {
    if let Some(parent) = args.cache.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut db = Connection::open(&args.cache)?;
    db.busy_timeout(std::time::Duration::from_secs(5))?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, provider TEXT, size INTEGER, mtime INTEGER, version INTEGER, rows TEXT, malformed INTEGER);")?;
    let mut tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let roots = [
        ("codex", args.codex_dir.join("sessions")),
        ("codex", args.codex_dir.join("archived_sessions")),
        ("claude", args.claude_dir.join("projects")),
        ("dsh", args.dsh_dir.join("sessions")),
    ];
    let mut meta = Metadata::default();
    let mut result = HashMap::new();
    let mut paths = HashSet::new();
    for (provider, root) in roots {
        if !args.provider.matches(provider) {
            continue;
        }
        if !root.exists() {
            // DSH is optional; only report it when explicitly requested.
            if provider == "dsh" && args.provider == Provider::All {
                continue;
            }
            meta.warnings
                .push(format!("Missing log directory: {}", root.display()));
            continue;
        }
        for entry in WalkDir::new(&root).into_iter() {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    meta.warnings.push(e.to_string());
                    continue;
                }
            };
            let path = entry.path();
            if !entry.file_type().is_file() || !is_log(path, provider) {
                continue;
            }
            let absolute = path.canonicalize()?;
            if !paths.insert(absolute.clone()) {
                continue;
            }
            let stat = path.metadata()?;
            let mtime = stat.modified()?.duration_since(UNIX_EPOCH)?.as_nanos() as i64;
            let saved:Option<(i64,i64,i64,String,usize)>=tx.query_row("SELECT size,mtime,version,rows,malformed FROM files WHERE path=? AND provider=?",params![absolute.to_string_lossy(),provider],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get::<_,i64>(4)? as usize))).optional()?;
            let cached = saved.filter(|s| {
                !args.refresh && (s.0, s.1, s.2) == (stat.len() as i64, mtime, CACHE_VERSION)
            });
            let decoded = cached.and_then(|s| {
                serde_json::from_str::<Vec<Record>>(&s.3)
                    .ok()
                    .map(|rows| (rows, s.4))
            });
            let (rows, bad) = if let Some(value) = decoded {
                meta.cached_files += 1;
                value
            } else {
                let (rows, bad) = match parse_file(path, provider) {
                    Ok(v) => v,
                    Err(e) => {
                        meta.warnings.push(format!("{}: {e}", path.display()));
                        continue;
                    }
                };
                tx.execute(
                    "INSERT OR REPLACE INTO files VALUES (?,?,?,?,?,?,?)",
                    params![
                        absolute.to_string_lossy(),
                        provider,
                        stat.len() as i64,
                        mtime,
                        CACHE_VERSION,
                        serde_json::to_string(&rows)?,
                        bad as i64
                    ],
                )?;
                meta.parsed_files += 1;
                (rows, bad)
            };
            if bad > 0 {
                meta.warnings.push(format!(
                    "{}: {bad} malformed/incomplete lines skipped",
                    path.display()
                ));
            }
            for r in rows {
                let k = r.key();
                if result
                    .get(&k)
                    .is_none_or(|prev: &Record| r.timestamp > prev.timestamp)
                {
                    result.insert(k, r);
                }
            }
            let count = meta.parsed_files + meta.cached_files;
            if count % 64 == 0 {
                progress(meta.parsed_files, meta.cached_files);
            }
            if meta.parsed_files > 0 && meta.parsed_files % 64 == 0 && count % 64 == 0 {
                tx.commit()?;
                tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            }
        }
    }
    tx.commit()?;
    progress(meta.parsed_files, meta.cached_files);
    let mut rows: Vec<_> = result.into_values().collect();
    rows.sort_by(|a, b| a.timestamp.total_cmp(&b.timestamp));
    Ok((rows, meta))
}

#[derive(Clone, Copy, Debug)]
pub struct Clock(pub Option<Tz>);
impl Clock {
    pub fn new(name: Option<&str>) -> Result<Self> {
        Ok(Self(
            name.map(|s| s.parse::<Tz>())
                .transpose()
                .context("Invalid IANA timezone")?,
        ))
    }
    pub fn label(self) -> String {
        self.0
            .map(|z| z.name().to_owned())
            .unwrap_or_else(|| Local::now().format("%Z").to_string())
    }
    pub fn format(self, stamp: f64, fmt: &str) -> String {
        let d = DateTime::<Utc>::from_timestamp_millis((stamp * 1000.0) as i64).unwrap_or_default();
        match self.0 {
            Some(tz) => d.with_timezone(&tz).format(fmt).to_string(),
            None => d.with_timezone(&Local).format(fmt).to_string(),
        }
    }
    pub fn boundary(self, s: &str, end: bool) -> Result<f64> {
        if let Ok(d) = DateTime::parse_from_rfc3339(s) {
            return Ok(d.timestamp_millis() as f64 / 1000.0);
        }
        let d = if s.len() == 10 {
            let date = NaiveDate::parse_from_str(s, "%Y-%m-%d").context("Invalid date")?;
            let date = if end { date + Duration::days(1) } else { date };
            date.and_hms_opt(0, 0, 0).unwrap()
        } else {
            NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
                .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M"))
                .context("Invalid ISO time")?
        };
        let stamp = match self.0 {
            Some(tz) => tz.from_local_datetime(&d).single().map(|d| d.timestamp()),
            None => Local
                .from_local_datetime(&d)
                .single()
                .map(|d| d.timestamp()),
        };
        stamp
            .map(|t| t as f64)
            .context("Ambiguous or nonexistent local time; supply an explicit UTC offset")
    }
}
pub fn bounds(args: &Args, clock: Clock) -> Result<(Option<f64>, Option<f64>)> {
    let now = Utc::now().timestamp_millis() as f64 / 1000.0;
    let since = if let Some(s) = &args.since {
        Some(clock.boundary(s, false)?)
    } else if args.today {
        Some(clock.boundary(&clock.format(now, "%Y-%m-%d"), false)?)
    } else if args.all_time {
        None
    } else {
        Some(now - args.days as f64 * 86400.0)
    };
    let until = args
        .until
        .as_deref()
        .map(|s| clock.boundary(s, true))
        .transpose()?;
    if matches!((since,until),(Some(s),Some(e)) if e<=s) {
        bail!("--until must be after --since");
    }
    Ok((since, until))
}
pub fn select(rows: &[Record], args: &Args, clock: Clock) -> Result<Vec<Record>> {
    let (since, until) = bounds(args, clock)?;
    Ok(rows
        .iter()
        .filter(|r| {
            args.provider.matches(&r.provider)
                && since.is_none_or(|s| r.timestamp >= s)
                && until.is_none_or(|u| r.timestamp < u)
                && args
                    .model
                    .as_ref()
                    .is_none_or(|m| r.model.to_lowercase().contains(&m.to_lowercase()))
                && args.session.as_ref().is_none_or(|s| r.session.contains(s))
        })
        .cloned()
        .collect())
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Summary {
    pub period: String,
    pub provider: String,
    pub model: String,
    pub requests: usize,
    pub timed_requests: usize,
    pub input_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub timed_output_tokens: u64,
    pub seconds: f64,
    pub tokens_per_second: Option<f64>,
    pub median_tokens_per_second: Option<f64>,
}
pub fn summarize(
    rows: impl IntoIterator<Item = impl std::borrow::Borrow<Record>>,
    max_gap: f64,
) -> Summary {
    let mut s = Summary::default();
    let mut rates = Vec::new();
    for r in rows {
        let r = r.borrow();
        s.requests += 1;
        s.input_tokens += r.input_tokens;
        s.cached_tokens += r.cached_tokens;
        s.cache_write_tokens += r.cache_write_tokens;
        s.output_tokens += r.output_tokens;
        s.reasoning_tokens += r.reasoning_tokens;
        if let Some(d) = r.duration(max_gap) {
            s.timed_requests += 1;
            s.seconds += d;
            s.timed_output_tokens += r.output_tokens;
            rates.push(r.output_tokens as f64 / d);
        }
    }
    if s.seconds > 0.0 {
        s.tokens_per_second = Some(s.timed_output_tokens as f64 / s.seconds);
    }
    rates.sort_by(f64::total_cmp);
    let len = rates.len();
    if len > 0 {
        s.median_tokens_per_second = Some(if len % 2 == 0 {
            (rates[len / 2 - 1] + rates[len / 2]) / 2.0
        } else {
            rates[len / 2]
        });
    }
    s
}
pub fn aggregate(rows: &[Record], group: Group, clock: Clock, max_gap: f64) -> Vec<Summary> {
    let mut bins: BTreeMap<(String, String, String), Vec<&Record>> = BTreeMap::new();
    for r in rows {
        let period = match group {
            Group::Hour => clock.format(r.timestamp, "%Y-%m-%d %H:00 %z"),
            Group::Day => clock.format(r.timestamp, "%Y-%m-%d"),
            Group::Model => "all".into(),
        };
        bins.entry((period, r.provider.clone(), r.model.clone()))
            .or_default()
            .push(r);
    }
    bins.into_iter()
        .map(|((period, provider, model), rs)| Summary {
            period,
            provider,
            model,
            ..summarize(rs, max_gap)
        })
        .collect()
}
