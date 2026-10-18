//! Per-source log parsers. Each turns one log file into usage `Record`s;
//! `docs/metrics.md` describes the timing and normalization each applies.

pub mod claude;
pub mod codex;
pub mod dsh;

use crate::data::Record;
use anyhow::Result;
use chrono::{DateTime, NaiveDateTime};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

/// Records parsed from one file, and the count of malformed or incomplete lines.
pub type Parsed = (Vec<Record>, usize);

pub fn is_jsonl(name: &str) -> bool {
    name.ends_with(".jsonl")
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

/// Call `each` with every JSON object line; returns the malformed count.
fn json_lines(path: &Path, mut each: impl FnMut(usize, Value)) -> Result<usize> {
    let mut stream = BufReader::with_capacity(256 * 1024, File::open(path)?);
    let mut line = Vec::new();
    let (mut index, mut malformed) = (0, 0);
    loop {
        line.clear();
        if stream.read_until(b'\n', &mut line)? == 0 {
            return Ok(malformed);
        }
        index += 1;
        match serde_json::from_slice::<Value>(&line) {
            Ok(v) if v.is_object() => each(index, v),
            _ => malformed += 1,
        }
    }
}

/// Keep the maximum of each counter when a response is logged more than once
/// (streamed blocks, mirrors), so repeated usage is never added.
fn merge_max(prev: &mut Record, r: &Record) {
    prev.timestamp = prev.timestamp.max(r.timestamp);
    prev.input_tokens = prev.input_tokens.max(r.input_tokens);
    prev.output_tokens = prev.output_tokens.max(r.output_tokens);
    prev.cached_tokens = prev.cached_tokens.max(r.cached_tokens);
    prev.cache_write_tokens = prev.cache_write_tokens.max(r.cache_write_tokens);
    prev.reasoning_tokens = prev.reasoning_tokens.max(r.reasoning_tokens);
    if r.speed_mode.is_some() {
        prev.speed_mode.clone_from(&r.speed_mode);
    }
}

/// One record per key, the last one winning, sorted by completion time.
fn finish(rows: impl IntoIterator<Item = Record>) -> Vec<Record> {
    let mut unique = HashMap::new();
    for r in rows {
        unique.insert(r.key(), r);
    }
    let mut rows: Vec<_> = unique.into_values().collect();
    rows.sort_by(|a, b| a.timestamp.total_cmp(&b.timestamp));
    rows
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
