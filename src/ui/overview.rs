//! Overview: per-source cards, activity map, latest requests, leaderboard.

use super::{
    theme::{bold, style},
    widgets::*,
};
use crate::{
    app::{App, Click, Range},
    args::Screen,
    source::Source,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Gauge, Paragraph, Sparkline, Wrap},
};

pub(super) fn leaderboard(f: &mut Frame, area: Rect, app: &App) {
    let pal = app.palette();
    let b = pal.block(" MODEL VELOCITY ");
    let inner = b.inner(area);
    f.render_widget(b, area);
    if inner.height < 2 {
        return;
    }
    let mut models = app.view.models.clone();
    models.sort_by(|a, b| {
        b.tokens_per_second
            .unwrap_or(0.0)
            .total_cmp(&a.tokens_per_second.unwrap_or(0.0))
    });
    let max = models
        .first()
        .and_then(|m| m.tokens_per_second)
        .unwrap_or(1.0)
        .max(1.0);
    // A model takes a label row, a bar row and a spacer. Without room for all
    // of them the spacer goes; if that still is not enough, the last row says
    // how many are left out instead of dropping the slowest silently.
    let rows = inner.height as usize;
    let stride: u16 = if models.len() * 3 <= rows { 3 } else { 2 };
    let fits = rows / stride as usize;
    let shown = if models.len() > fits {
        fits.saturating_sub(1)
    } else {
        models.len()
    };
    for (i, m) in models.iter().take(shown).enumerate() {
        let y = inner.y + i as u16 * stride;
        let name = short_model(&m.model);
        let label = Line::from(vec![
            Span::styled(
                format!(
                    "{} ",
                    m.provider.map_or("ALL", Source::label).to_uppercase()
                ),
                style(pal.accent(m.provider)),
            ),
            Span::styled(name, style(pal.ink)),
        ]);
        let columns = Layout::horizontal([Constraint::Min(10), Constraint::Length(12)])
            .split(Rect::new(inner.x, y, inner.width, 1));
        f.render_widget(Paragraph::new(label), columns[0]);
        f.render_widget(
            Paragraph::new(format!("{} tok/s", rate(m.tokens_per_second)))
                .style(bold(pal.accent(m.provider)))
                .alignment(Alignment::Right),
            columns[1],
        );
        f.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(pal.accent(m.provider)).bg(pal.edge))
                .ratio((m.tokens_per_second.unwrap_or(0.0) / max).clamp(0.0, 1.0))
                .label("")
                .use_unicode(true),
            Rect::new(inner.x, y + 1, inner.width, 1),
        );
    }
    if models.len() > shown {
        let y = inner.y + shown as u16 * stride;
        if y < inner.bottom() {
            f.render_widget(
                Paragraph::new(format!("+{} more · Models tab", models.len() - shown))
                    .style(style(pal.muted)),
                Rect::new(inner.x, y, inner.width, 1),
            );
        }
    }
    if models.is_empty() {
        f.render_widget(
            Paragraph::new("Waiting for local telemetry…").style(style(pal.muted)),
            inner,
        );
    }
}

pub(super) fn activity(f: &mut Frame, area: Rect, app: &App) {
    let pal = app.palette();
    let b = pal.block(" OUTPUT ACTIVITY ");
    let inner = b.inner(area);
    f.render_widget(b, area);
    let shared_max = app
        .view
        .activity
        .iter()
        .flatten()
        .copied()
        .max()
        .unwrap_or(1)
        .max(1);
    let mut y = inner.y;
    for p in app.visible_sources() {
        let (label, color) = pal.source(p);
        let total = app.view.sources[p].output_tokens;
        if y + 1 >= inner.bottom() {
            break;
        }
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("{label}  "), style(color)),
                Span::styled(format!("{} tokens", compact(total)), style(pal.muted)),
            ])),
            Rect::new(inner.x, y, inner.width, 1),
        );
        let values: Vec<u64> = app.view.activity.iter().map(|v| v[p]).collect();
        // Resample to use the available width rather than leaving a short trace.
        let expanded: Vec<u64> = (0..inner.width as usize)
            .map(|i| {
                values
                    .get(i * values.len() / inner.width.max(1) as usize)
                    .copied()
                    .unwrap_or(0)
            })
            .collect();
        f.render_widget(
            Sparkline::default()
                .data(&expanded)
                .max(shared_max)
                .style(style(color)),
            Rect::new(inner.x, y + 1, inner.width, 1),
        );
        y += 2;
    }
}

pub(super) fn overview(f: &mut Frame, area: Rect, app: &mut App) {
    let rows = Layout::vertical([
        Constraint::Length(if area.height >= 24 { 12 } else { 8 }),
        Constraint::Min(4),
    ])
    .spacing(1)
    .split(area);
    let sources = app.visible_sources();
    let n = sources.len() as u32;
    let cards = Layout::horizontal((0..n).map(|_| Constraint::Ratio(1, n)))
        .spacing(1)
        .split(rows[0]);
    for (i, p) in sources.into_iter().enumerate() {
        provider_card(f, cards[i], app, p);
    }
    if area.width >= 105 && rows[1].height >= 11 {
        let halves = Layout::horizontal([Constraint::Percentage(65), Constraint::Percentage(35)])
            .spacing(1)
            .split(rows[1]);
        let left = Layout::vertical([Constraint::Length(9), Constraint::Min(4)]).split(halves[0]);
        coverage_panel(f, left[0], app);
        overview_note(f, left[1], app);
        // Activity needs two rows per source plus its border; velocity gets
        // the rest and marks any models it has no room for.
        let need =
            (app.visible_sources().len() as u16 * 2 + 2).min(halves[1].height.saturating_sub(8));
        let right =
            Layout::vertical([Constraint::Min(5), Constraint::Length(need)]).split(halves[1]);
        leaderboard(f, right[0], app);
        activity(f, right[1], app);
    } else {
        coverage_panel(f, rows[1], app);
    }
}

pub(super) fn provider_card(f: &mut Frame, area: Rect, app: &App, p: Source) {
    let pal = app.palette();
    let (name, color) = pal.source(p);
    let s = &app.view.sources[p];
    let b = pal
        .block(Line::from(vec![
            Span::styled(format!(" {} ", name.to_uppercase()), bold(color)),
            Span::styled(" weighted request throughput ", style(pal.muted)),
        ]))
        .border_style(style(color));
    let inner = b.inner(area);
    f.render_widget(b, area);
    if !app.args.provider.matches(p) {
        f.render_widget(
            Paragraph::new(vec![
                Line::styled("—", bold(color)),
                Line::raw(""),
                Line::styled("Hidden by source filter", style(pal.muted)),
                Line::styled("Press 1 to compare all", style(pal.muted)),
            ]),
            inner,
        );
        return;
    }
    let value = rate(s.tokens_per_second);
    let big = inner.height >= 6 && inner.width >= value.chars().count() as u16 * 4;
    let mut lines = if big {
        big_number(&value, color)
    } else {
        vec![Line::styled(format!("{value} tok/s"), bold(color))]
    };
    if big {
        lines.push(Line::styled(
            "estimated output tokens / second",
            style(pal.muted),
        ));
    }
    lines.push(Line::from(vec![
        Span::styled(
            format!("Median {}", rate(s.median_tokens_per_second)),
            style(pal.ink),
        ),
        Span::styled(format!("  ·  {} requests", s.requests), style(pal.muted)),
    ]));
    let usable = if s.requests > 0 {
        format!(
            "{:.0}% usable timing",
            s.timed_requests as f64 / s.requests as f64 * 100.0
        )
    } else {
        "No requests in this window".into()
    };
    lines.push(Line::styled(
        format!("{} output tokens · {usable}", compact(s.output_tokens)),
        style(pal.muted),
    ));
    let height = lines.len() as u16;
    f.render_widget(
        Paragraph::new(lines),
        Rect::new(inner.x, inner.y, inner.width, inner.height.min(height)),
    );
    if inner.height > height + 1 {
        let values: Vec<u64> = app.view.activity.iter().map(|v| v[p]).collect();
        let expanded: Vec<u64> = (0..inner.width as usize)
            .map(|i| {
                values
                    .get(i * values.len() / inner.width.max(1) as usize)
                    .copied()
                    .unwrap_or(0)
            })
            .collect();
        let h = inner.height.saturating_sub(height + 1).min(2);
        f.render_widget(
            Sparkline::default().data(&expanded).style(style(color)),
            Rect::new(inner.x, inner.bottom() - h, inner.width, h),
        );
    }
}

pub(super) fn coverage_line(f: &mut Frame, area: Rect, app: &mut App, p: Source, legend: bool) {
    let pal = app.palette();
    if area.height == 0 {
        return;
    }
    let (name, color) = pal.source(p);
    let mut spans = vec![Span::styled(format!("{name:<10}"), style(color))];
    if !app.args.provider.matches(p) {
        spans.push(Span::styled("hidden by source filter", style(pal.muted)));
    } else if app.view.bins.is_empty() {
        spans.push(Span::styled("Waiting for local data…", style(pal.muted)));
    } else {
        let cells = area.width.saturating_sub(10) as usize;
        let count = app.view.bins.len();
        let max = app
            .view
            .activity
            .iter()
            .map(|a| a[p])
            .max()
            .unwrap_or(1)
            .max(1);
        let shades = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        for x in 0..cells {
            let i = x * count / cells.max(1);
            app.hits.push((
                Rect::new(area.x + 10 + x as u16, area.y, 1, 1),
                Click::Bucket(i),
            ));
            let (symbol, c) = if app.view.requests[i][p] == 0 {
                ('░', pal.muted)
            } else if app.view.timed[i][p] == 0 {
                ('!', pal.gold)
            } else {
                (
                    shades[(app.view.activity[i][p] as f64 / max as f64 * 7.0).round() as usize],
                    color,
                )
            };
            let bg = if app.screen == Screen::Timeline && i == app.cursor {
                pal.inspected
            } else {
                pal.panel
            };
            spans.push(Span::styled(symbol.to_string(), style(c).bg(bg)));
        }
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect::new(area.x, area.y, area.width, 1),
    );
    if legend && area.height > 1 {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("       ░ no requests", style(pal.muted)),
                Span::styled("  ! no usable timing", style(pal.gold)),
                Span::styled("  ▁..█ output volume", style(color)),
            ])),
            Rect::new(area.x, area.y + 1, area.width, 1),
        );
    }
}

pub(super) fn coverage_panel(f: &mut Frame, area: Rect, app: &mut App) {
    let pal = app.palette();
    let b = pal.block(" ACTIVITY MAP  ·  activity by time bucket ");
    let inner = b.inner(area);
    f.render_widget(b, area);
    if inner.height < 2 {
        return;
    }
    let sources = app.visible_sources();
    let n = sources.len() as u16;
    if inner.height < n + 2 {
        for (i, &p) in sources.iter().enumerate() {
            if (i as u16) < inner.height {
                coverage_line(
                    f,
                    Rect::new(inner.x, inner.y + i as u16, inner.width, 1),
                    app,
                    p,
                    false,
                );
            }
        }
        if inner.height > n {
            f.render_widget(
                Paragraph::new("░ no requests · ! no timing").style(style(pal.muted)),
                Rect::new(inner.x, inner.y + n, inner.width, 1),
            );
        }
        return;
    }
    let fmt = if app.range == Range::Today {
        "%H:%M"
    } else {
        "%m/%d %Hh"
    };
    let start = app.clock.format(app.view.chart_start, fmt);
    let end = app.clock.format(
        app.view.chart_start
            + app.view.bins.len().saturating_sub(1) as f64 * app.view.bucket_seconds,
        fmt,
    );
    f.render_widget(
        Paragraph::new(format!("{start}  →  {end}")).style(style(pal.muted)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    for (i, &p) in sources.iter().enumerate() {
        coverage_line(
            f,
            Rect::new(inner.x, inner.y + 1 + i as u16, inner.width, 1),
            app,
            p,
            false,
        );
    }
    let legend = n + 2;
    if inner.height > legend {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("░ no requests  ", style(pal.muted)),
                    Span::styled("! no usable timing  ", style(pal.gold)),
                    Span::styled("▁..█ output volume", style(pal.mint)),
                ]),
                Line::styled(
                    "A quiet period is missing activity, not a measured speed of zero.",
                    style(pal.muted),
                ),
            ])
            .wrap(Wrap { trim: true }),
            Rect::new(
                inner.x,
                inner.y + legend,
                inner.width,
                inner.height - legend,
            ),
        );
    }
}

pub(super) fn overview_note(f: &mut Frame, area: Rect, app: &App) {
    let pal = app.palette();
    let b = pal.block(" LATEST ACTIVITY ");
    let inner = b.inner(area);
    f.render_widget(b, area);
    if inner.height == 0 {
        return;
    }
    let sessions = app
        .view
        .rows
        .iter()
        .map(|r| (&r.provider, &r.session))
        .collect::<std::collections::HashSet<_>>()
        .len();
    f.render_widget(
        Paragraph::new(format!(
            "{sessions} sessions · {} models · {} output tokens",
            app.view.models.len(),
            compact(app.view.totals.output_tokens)
        ))
        .style(style(pal.muted)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    for (i, r) in app
        .view
        .rows
        .iter()
        .rev()
        .take(inner.height.saturating_sub(2) as usize)
        .enumerate()
    {
        let line = Line::from(vec![
            Span::styled(
                app.clock.format(r.timestamp, "%m/%d %H:%M:%S"),
                style(pal.muted),
            ),
            Span::styled(
                format!("  {:<9}", r.provider),
                style(pal.accent(r.provider)),
            ),
            Span::styled(
                format!("  {} tok/s", rate(r.rate(app.args.max_gap))),
                bold(pal.accent(r.provider)),
            ),
            Span::styled(
                format!(
                    "  {} output  ·  {}",
                    compact(r.output_tokens),
                    short_model(&r.model)
                ),
                style(pal.muted),
            ),
        ]);
        f.render_widget(
            Paragraph::new(line),
            Rect::new(inner.x, inner.y + 2 + i as u16, inner.width, 1),
        );
    }
}
