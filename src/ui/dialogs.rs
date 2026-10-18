//! Help, detail and filter dialogs.

use super::{
    theme::{bold, style},
    timeline::bucket_lines,
    widgets::*,
};
use crate::{
    app::App,
    args::Screen,
    data::Record,
    source::{Filter, Source},
};
use ratatui::{Frame, text::Line};

/// "1 / 2 / 3 / 4  All / Codex / Claude / DSH", from the source registry.
fn source_keys() -> String {
    let filters = std::iter::once(Filter::ALL).chain(Source::ALL.map(Filter::only));
    let (keys, names): (Vec<_>, Vec<_>) =
        filters.map(|f| (f.digit().to_string(), f.label())).unzip();
    format!("{:<14}{}", keys.join(" / "), names.join(" / "))
}

pub(super) fn help(f: &mut Frame, app: &App) {
    let pal = app.palette();
    let mut lines = vec![
        Line::styled("YOUR TERMINAL, YOUR TELEMETRY", bold(pal.mint)),
        Line::raw(""),
        Line::raw(source_keys()),
        Line::raw("d             Cycle the time window"),
        Line::raw("T             Toggle dark / light theme"),
        Line::raw("t / w / m / a Today / 7 days / 30 days / All time"),
        Line::raw("Tab / Shift-Tab Cycle views · F1–F4 direct selection"),
        Line::raw("Timeline: ← → inspect bucket · g split / overlay"),
        Line::raw("↑ ↓ or j k    Select a row · Enter opens details"),
        Line::raw("/             Filter model names and session IDs"),
        Line::raw("Esc           Clear filter / close dialog"),
        Line::raw("Space         Pause live updates"),
        Line::raw("r             Refresh · q or Ctrl-C quits"),
        Line::raw(""),
        Line::styled("HOW TO READ THE CHART", bold(pal.purple)),
        Line::raw("Output tokens ÷ estimated request seconds. Weighted by time."),
        Line::raw("Includes prefill, reasoning, network and residual waiting."),
        Line::raw("░ no requests · ! missing timing · colored bars = activity"),
        Line::raw("Missing intervals break the line; they are not zero speed."),
        Line::raw("Provider comparisons reflect different workloads."),
        Line::raw(""),
        Line::styled(
            format!("Local only · {} · no model API calls", app.clock.label()),
            style(pal.muted),
        ),
    ];
    if let Some(w) = app.meta.warnings.first() {
        lines.push(Line::styled(format!("Warning: {w}"), style(pal.gold)));
    }
    lines.push(Line::styled(
        "\nPress Esc or Enter to return",
        style(pal.muted),
    ));
    popup(pal, f, 74, 27, " KEYBOARD & METRICS ", lines);
}

pub(super) fn detail(f: &mut Frame, app: &App) {
    let pal = app.palette();
    if app.screen == Screen::Timeline {
        popup(pal, f, 78, 14, " TIME BUCKET ", bucket_lines(app));
        return;
    }
    let Some(i) = app.table.selected() else {
        return;
    };
    let lines = if app.screen != Screen::Requests {
        let Some(m) = app.view.models.get(i) else {
            return;
        };
        vec![
            Line::styled(m.model.clone(), bold(pal.accent(m.provider))),
            Line::raw(""),
            Line::raw(format!(
                "Source         {}",
                m.provider.map_or("all", Source::id)
            )),
            Line::raw(format!("Requests       {}", m.requests)),
            Line::raw(format!("Output         {} tokens", m.output_tokens)),
            Line::raw(format!("Input          {} tokens", m.input_tokens)),
            Line::raw(format!("Cached input   {} tokens", m.cached_tokens)),
            Line::raw(format!(
                "Reasoning      {} tokens (included in output)",
                m.reasoning_tokens
            )),
            Line::raw(format!(
                "Weighted speed {} tok/s",
                rate(m.tokens_per_second)
            )),
            Line::raw(format!(
                "Median speed   {} tok/s",
                rate(m.median_tokens_per_second)
            )),
            Line::raw(format!(
                "Usable timing  {} / {} requests",
                m.timed_requests, m.requests
            )),
            Line::raw(""),
            Line::styled("Esc / Enter to return", style(pal.muted)),
        ]
    } else {
        let Some(r) = app.view.rows.iter().rev().nth(i) else {
            return;
        };
        record_detail(r, app)
    };
    popup(pal, f, 76, 21, " INSPECT ", lines);
}

pub(super) fn record_detail(r: &Record, app: &App) -> Vec<Line<'static>> {
    let pal = app.palette();
    vec![
        Line::styled(r.model.clone(), bold(pal.accent(r.provider))),
        Line::raw(""),
        Line::raw(format!(
            "Completed   {}",
            app.clock.format(r.timestamp, "%Y-%m-%d %H:%M:%S %Z")
        )),
        Line::raw(format!("Output      {} tokens", r.output_tokens)),
        Line::raw(format!("Input       {} tokens", r.input_tokens)),
        Line::raw(format!("Cached      {} tokens", r.cached_tokens)),
        Line::raw(format!(
            "Reasoning   {} tokens (included in output)",
            r.reasoning_tokens
        )),
        Line::raw(format!(
            "Wait        {}",
            r.duration(app.args.max_gap).map_or_else(
                || "Missing or excluded interval".into(),
                |d| format!("{d:.3}s")
            )
        )),
        Line::raw(format!(
            "Est. speed  {} tok/s",
            rate(r.rate(app.args.max_gap))
        )),
        Line::raw(format!("Timing      {}", r.timing_source)),
        Line::raw(format!(
            "Speed mode  {}",
            r.speed_mode.as_deref().unwrap_or("not logged")
        )),
        Line::raw(format!("Session     {}", r.session)),
        Line::raw(format!("Response    {}", r.id)),
        Line::raw(""),
        Line::styled("Esc / Enter to return", style(pal.muted)),
    ]
}

pub(super) fn search(f: &mut Frame, app: &App) {
    let pal = app.palette();
    popup(
        pal,
        f,
        68,
        7,
        " FILTER ",
        vec![
            Line::styled(format!("› {}▏", app.search), bold(pal.mint)),
            Line::raw(""),
            Line::styled(
                "Model or session · Enter applies · Esc clears",
                style(pal.muted),
            ),
        ],
    );
}
