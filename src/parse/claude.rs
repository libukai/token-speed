//! Claude Code project logs. Assistant blocks are merged by message; a
//! request starts at the nearest user event or previous response on the same
//! `parentUuid` branch. Input is normalized to include cache reads / writes.

use super::{Parsed, file_stem, finish, json_lines, merge_max, n, stamp, text};
use crate::{data::Record, source::Source};
use anyhow::Result;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

type MessageKey = (String, String);

struct Node {
    kind: String,
    time: Option<f64>,
    parent: Option<String>,
    message: MessageKey,
}

fn record(session: &str, id: &str, model: &str, end: f64, u: &Value) -> Record {
    let cached = n(u, "cache_read_input_tokens");
    let written = n(u, "cache_creation_input_tokens");
    Record {
        provider: Source::Claude,
        session: session.into(),
        id: id.into(),
        model: model.into(),
        timestamp: end,
        start: None,
        input_tokens: n(u, "input_tokens") + cached + written,
        cached_tokens: cached,
        cache_write_tokens: written,
        output_tokens: n(u, "output_tokens"),
        reasoning_tokens: n(&u["output_tokens_details"], "thinking_tokens"),
        timing_source: "parent-boundary".into(),
        speed_mode: None,
    }
}

pub fn parse(path: &Path) -> Result<Parsed> {
    let session = file_stem(path);
    let mut messages: HashMap<MessageKey, Record> = HashMap::new();
    let mut parents: HashMap<MessageKey, Option<String>> = HashMap::new();
    let mut nodes: HashMap<String, Node> = HashMap::new();
    let malformed = json_lines(path, |index, d| {
        let t = stamp(&d["timestamp"]);
        let typ = text(&d["type"]).unwrap_or("");
        let sid = text(&d["sessionId"])
            .or_else(|| text(&d["session_id"]))
            .unwrap_or(&session)
            .to_owned();
        if let Some(uuid) = text(&d["uuid"]) {
            nodes.insert(
                uuid.to_owned(),
                Node {
                    kind: typ.to_owned(),
                    time: t,
                    parent: text(&d["parentUuid"]).map(String::from),
                    message: (
                        sid.clone(),
                        text(&d["message"]["id"]).unwrap_or("").to_owned(),
                    ),
                },
            );
        }
        if typ != "assistant"
            || t.is_none()
            || !d["message"]["usage"].is_object()
            || d["message"]["model"] == "<synthetic>"
        {
            return;
        }
        let fallback = format!("line-{index}");
        let mid = text(&d["message"]["id"])
            .or_else(|| text(&d["requestId"]))
            .or_else(|| text(&d["uuid"]))
            .unwrap_or(&fallback);
        let key = (sid.clone(), mid.to_owned());
        let r = record(
            &sid,
            mid,
            text(&d["message"]["model"]).unwrap_or("unknown"),
            t.unwrap(),
            &d["message"]["usage"],
        );
        if let Some(prev) = messages.get_mut(&key) {
            merge_max(prev, &r);
        } else {
            parents.insert(key.clone(), text(&d["parentUuid"]).map(String::from));
            messages.insert(key, r);
        }
    })?;
    let rows = messages.iter().map(|(key, r)| {
        let mut r = r.clone();
        r.start = start(key, &parents, &nodes, &messages);
        r
    });
    Ok((finish(rows), malformed))
}

/// Walk ancestors on the same branch to the event that started this request.
fn start(
    key: &MessageKey,
    parents: &HashMap<MessageKey, Option<String>>,
    nodes: &HashMap<String, Node>,
    messages: &HashMap<MessageKey, Record>,
) -> Option<f64> {
    let mut parent = parents.get(key).cloned().flatten();
    let mut seen = HashSet::new();
    while let Some(id) = parent {
        if !seen.insert(id.clone()) {
            return None;
        }
        let node = nodes.get(&id)?;
        if node.kind == "user" {
            return node.time;
        }
        if node.kind == "assistant" && &node.message != key {
            // The previous response ends at its last block.
            return messages
                .get(&node.message)
                .map(|p| p.timestamp)
                .or(node.time);
        }
        parent = node.parent.clone();
    }
    None
}
