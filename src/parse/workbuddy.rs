//! WorkBuddy request logs. When a model request ends, the app writes one
//! `[ModelProvider] [SseMonitor#…] qua.end: {…}` line to `logs/<day>/*.log`
//! with the request's token counts and its total time.
//!
//! The line carries no cache counters, so cache stays zero. Its reasoning
//! and text counts are estimated from content and do not add up to
//! `completion_tokens`, so no reasoning is recorded.

use super::{Parsed, finish, text};
use crate::{data::Record, source::Source};
use anyhow::Result;
use chrono::{Local, NaiveDateTime, TimeZone};
use serde_json::Value;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

const MARKER: &[u8] = b"qua.end:";

pub fn is_log(name: &str) -> bool {
    name.ends_with(".log")
}

pub fn parse(path: &Path) -> Result<Parsed> {
    let mut stream = BufReader::with_capacity(256 * 1024, File::open(path)?);
    let session = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let (mut rows, mut malformed) = (Vec::new(), 0);
    let (mut line, mut index) = (Vec::new(), 0);
    loop {
        line.clear();
        if stream.read_until(b'\n', &mut line)? == 0 {
            return Ok((finish(rows), malformed));
        }
        index += 1;
        let Some(at) = line.windows(MARKER.len()).position(|w| w == MARKER) else {
            continue;
        };
        match record(&line, at + MARKER.len(), &session, index) {
            Some(Ok(r)) => rows.push(r),
            Some(Err(())) => malformed += 1,
            None => {}
        }
    }
}

/// `None` for a request that reported no usage; `Err` for a line that cannot
/// be read.
fn record(line: &[u8], marker_end: usize, file: &str, index: usize) -> Option<Result<Record, ()>> {
    let text_line = String::from_utf8_lossy(line);
    let end = stamp(&text_line).ok_or(());
    let body = &line[marker_end..];
    let json = body.iter().position(|b| *b == b'{').map(|i| &body[i..]);
    let d: Value = match json.and_then(|j| {
        serde_json::Deserializer::from_slice(j)
            .into_iter::<Value>()
            .next()
    }) {
        Some(Ok(v)) if v.is_object() => v,
        _ => return Some(Err(())),
    };
    let output = count(&d, "completion_tokens");
    let input = count(&d, "prompt_tokens");
    // A request that failed before streaming carries no usage.
    if input == 0 && output == 0 {
        return None;
    }
    let end = match end {
        Ok(t) => t,
        Err(()) => return Some(Err(())),
    };
    let seconds = count(&d, "total_cost_ms") as f64 / 1000.0;
    let id = text(&d["message_id"])
        .map(String::from)
        .unwrap_or_else(|| format!("{file}-{index}"));
    Some(Ok(Record {
        provider: Source::Workbuddy,
        session: text(&d["conversation_id"]).unwrap_or(file).into(),
        id,
        model: text(&d["model"]).unwrap_or("unknown").into(),
        timestamp: end,
        start: (seconds > 0.0).then_some(end - seconds),
        input_tokens: input,
        cached_tokens: 0,
        cache_write_tokens: 0,
        output_tokens: output,
        reasoning_tokens: 0,
        timing_source: "request-cost".into(),
        speed_mode: None,
    }))
}

/// Counters are logged as strings.
fn count(d: &Value, key: &str) -> u64 {
    match &d[key] {
        Value::String(s) => s.trim().parse().unwrap_or(0),
        v => v.as_u64().unwrap_or(0),
    }
}

/// The line's `[10/12/2026, 5:18:57 PM.186]` prefix is local time without
/// a zone. An hour the clock skipped has no timestamp.
fn stamp(line: &str) -> Option<f64> {
    let inner = line.strip_prefix('[')?.split_once(']')?.0;
    let (clock, millis) = inner.split_once('.').unwrap_or((inner, "0"));
    let naive = NaiveDateTime::parse_from_str(clock, "%m/%d/%Y, %I:%M:%S %p").ok()?;
    let local = Local.from_local_datetime(&naive).earliest()?;
    let millis: u32 = millis.get(..3.min(millis.len()))?.parse().ok()?;
    Some(local.timestamp() as f64 + f64::from(millis) / 1000.0)
}
