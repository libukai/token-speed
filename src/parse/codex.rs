//! Codex rollout logs. Requests start at a user / task start, a tool result
//! or the previous usage event. Input already includes cache.

use super::{Parsed, file_stem, finish, json_lines, n, stamp, text};
use crate::{data::Record, source::Source};
use anyhow::Result;
use serde_json::Value;
use std::path::Path;

fn record(session: &str, id: &str, model: &str, end: f64, start: Option<f64>, u: &Value) -> Record {
    Record {
        provider: Source::Codex,
        session: session.into(),
        id: id.into(),
        model: model.into(),
        timestamp: end,
        start,
        input_tokens: n(u, "input_tokens"),
        cached_tokens: n(u, "cached_input_tokens"),
        cache_write_tokens: n(u, "cache_write_input_tokens"),
        output_tokens: n(u, "output_tokens"),
        reasoning_tokens: n(u, "reasoning_output_tokens"),
        timing_source: "request-boundary".into(),
    }
}

pub fn parse(path: &Path) -> Result<Parsed> {
    let mut session = file_stem(path);
    let mut model = "unknown".to_owned();
    let mut anchor = None;
    let mut cumulative: Option<Value> = None;
    let (mut modern, mut legacy) = (Vec::new(), Vec::new());
    let malformed = json_lines(path, |index, d| {
        let t = stamp(&d["timestamp"]);
        let p = &d["payload"];
        let sub = text(&p["type"]).unwrap_or("");
        match text(&d["type"]).unwrap_or("") {
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
                // Parent logs mirror other threads' usage; skip it here.
                if text(&p["thread_id"]).is_some_and(|id| id != session) {
                    return;
                }
                let fallback = format!("line-{index}");
                modern.push(record(
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
                if !total.is_object() || !last.is_object() || cumulative.as_ref() == Some(total) {
                    return;
                }
                // Cumulative difference; after a counter reset, the last usage.
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
                legacy.push(record(
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
    })?;
    // Legacy events are ignored when the file has modern usage records.
    let rows = if modern.is_empty() { legacy } else { modern };
    Ok((finish(rows), malformed))
}
