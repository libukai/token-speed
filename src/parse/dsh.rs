//! DSH (DeepSeek Harness) session events. Each model call is a `step`,
//! bracketed by `step/start` and an `assistant/message` carrying its usage.
//! Sessions are zstd-compressed JSONL, appended one frame per write.

use super::{Parsed, finish, merge_max, n, stamp, text};
use crate::{data::Record, source::Source};
use anyhow::Result;
use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

pub fn is_log(name: &str) -> bool {
    super::is_jsonl(name) || name.ends_with(".jsonl.zstd") || name.ends_with(".jsonl.zst")
}

pub fn parse(path: &Path) -> Result<Parsed> {
    let raw = fs::read(path)?;
    let compressed = path.extension().is_some_and(|x| x == "zstd" || x == "zst");
    let (bytes, truncated) = if compressed {
        inflate(&raw)
    } else {
        (raw, false)
    };
    // Sessions live in session-<id>/session.jsonl.zstd; the header repeats the id.
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
                let id = text(&p["message"]["id"])
                    .map(String::from)
                    .unwrap_or_else(|| format!("turn-{}-step-{}", step.0, step.1));
                let r = record(&session, &id, p, end, steps.get(&step).copied());
                if let Some(prev) = messages.get_mut(&id) {
                    merge_max(prev, &r);
                } else {
                    messages.insert(id, r);
                }
            }
            _ => {}
        }
    }
    Ok((finish(messages.into_values()), malformed))
}

fn record(session: &str, id: &str, p: &Value, end: f64, start: Option<f64>) -> Record {
    let u = &p["usage"];
    let cached = n(u, "cacheReadTokens");
    let written = n(u, "cacheWriteTokens");
    Record {
        provider: Source::Dsh,
        session: session.into(),
        id: id.into(),
        model: text(&p["message"]["source"]["model"])
            .unwrap_or("unknown")
            .into(),
        timestamp: end,
        start,
        // DSH reports cache reads separately from uncached input.
        input_tokens: n(u, "inputTokens") + cached + written,
        cached_tokens: cached,
        cache_write_tokens: written,
        // Reasoning is already part of output.
        output_tokens: n(u, "outputTokens"),
        reasoning_tokens: n(u, "reasoningTokens"),
        timing_source: "step-boundary".into(),
    }
}

/// Decode concatenated zstd frames. A frame cut off mid-write is dropped
/// whole and reported, so the file is reparsed once the write completes.
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
