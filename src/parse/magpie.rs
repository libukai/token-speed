//! Magpie gateway ledger (`usage.jsonl`): one line per model call that went
//! through the gateway, whichever agent made it. It covers agents with no
//! log of their own, and the stretches where an agent's own log is missing.
//! A call is filed under the agent's source (`codex`, `claude`, `deepseek`, `workbuddy`) or,
//! for an agent token-speed has no parser for, under `other`, with the agent
//! named in the session.

use super::{Parsed, finish, json_lines, n, stamp, text};
use crate::{data::Record, source::Source};
use anyhow::Result;
use serde_json::Value;
use std::path::Path;

/// `timing_source` of records from this ledger. Indexing drops those that
/// duplicate a record from the agent's own log.
pub const GATEWAY: &str = "gateway";
/// Session of a call that names none of the agent's own.
pub const NO_SESSION: &str = "magpie";

pub fn parse(path: &Path) -> Result<Parsed> {
    let mut rows = Vec::new();
    let malformed = json_lines(path, |index, d| rows.extend(record(index, &d)))?;
    Ok((finish(rows), malformed))
}

fn record(index: usize, d: &Value) -> Option<Record> {
    let start = stamp(&d["t"])?;
    let cached = n(d, "cache_read");
    let written = n(d, "cache_write");
    // Gateway input excludes cache; the other sources count it in.
    let input = n(d, "in").saturating_add(cached).saturating_add(written);
    let output = n(d, "out");
    // Rejected or cancelled calls carry no usage.
    if input == 0 && output == 0 {
        return None;
    }
    let agent = text(&d["agent"]).unwrap_or("magpie");
    let provider = Source::from_id(agent).unwrap_or(Source::Other);
    let native = text(&d["native_session"]).or_else(|| text(&d["session"]));
    let session = match (provider, native) {
        (Source::Other, Some(s)) => format!("{agent}/{s}"),
        (Source::Other, None) => agent.into(),
        (_, Some(s)) => s.into(),
        (_, None) => NO_SESSION.into(),
    };
    // Without a call id, the line is the call: the ledger is append-only and
    // holds each call once.
    let id = text(&d["response_id"])
        .or_else(|| text(&d["rid"]))
        .map_or_else(|| format!("line-{index}"), String::from);
    let seconds = n(d, "ms") as f64 / 1000.0;
    Some(Record {
        provider,
        session,
        id,
        model: text(&d["model"]).unwrap_or("unknown").into(),
        // Calls are stamped when they begin; usage is attributed to the end.
        timestamp: start + seconds,
        start: (seconds > 0.0).then_some(start),
        input_tokens: input,
        cached_tokens: cached,
        cache_write_tokens: written,
        // Reasoning is already part of output.
        output_tokens: output,
        reasoning_tokens: n(d, "reasoning"),
        timing_source: GATEWAY.into(),
        speed_mode: None,
    })
}
