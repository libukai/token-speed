//! Models and Requests: tables with their side panels.

use super::{
    dialogs::record_detail,
    theme::{bold, style},
    widgets::*,
};
use crate::{
    app::{App, Click},
    args::Screen,
    source::Source,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::Style,
    symbols,
    text::{Line, Span},
    widgets::{Axis, Cell, Chart, Dataset, Gauge, GraphType, Paragraph, Row, Table, Wrap},
};

pub(super) fn records_table(f: &mut Frame, area: Rect, app: &mut App) {
    let pal = app.palette();
    let title = Line::from(vec![
        Span::styled(
            if app.screen == Screen::Requests {
                " REQUEST LOG "
            } else {
                " MODEL COMPARISON "
            },
            bold(pal.ink),
        ),
        Span::styled("  ↑/↓ select · Enter to inspect ", style(pal.muted)),
    ]);
    let header_style = style(pal.muted).bg(pal.panel);
    let (headers, widths, rows): (Vec<&str>, Vec<Constraint>, Vec<Row<'static>>) =
        if app.screen != Screen::Requests {
            let rows = app.view.models.iter().map(|m| {
                Row::new(vec![
                    Cell::from(m.provider.map_or("all", Source::id))
                        .style(style(pal.accent(m.provider))),
                    Cell::from(short_model(&m.model)),
                    Cell::from(grouped(m.requests)),
                    Cell::from(grouped(m.output_tokens)),
                    Cell::from(rate(m.tokens_per_second)).style(bold(pal.accent(m.provider))),
                    Cell::from(rate(m.median_tokens_per_second)),
                    Cell::from(format!(
                        "{}/{}",
                        grouped(m.timed_requests),
                        grouped(m.requests)
                    )),
                ])
            });
            (
                vec![
                    "Source",
                    "Model",
                    "Requests",
                    "Output",
                    "Est. tok/s",
                    "Median",
                    "Timed",
                ],
                vec![
                    Constraint::Length(8),
                    Constraint::Min(14),
                    count_column_width(
                        "Requests",
                        app.view.models.iter().map(|m| grouped(m.requests)),
                    ),
                    count_column_width(
                        "Output",
                        app.view.models.iter().map(|m| grouped(m.output_tokens)),
                    ),
                    Constraint::Length(10),
                    Constraint::Length(8),
                    count_column_width(
                        "Timed",
                        app.view.models.iter().map(|m| {
                            format!("{}/{}", grouped(m.timed_requests), grouped(m.requests))
                        }),
                    ),
                ],
                rows.collect(),
            )
        } else {
            let rows = app
                .view
                .rows
                .iter()
                .rev()
                .take(app.args.limit.max(200))
                .map(|r| {
                    Row::new(vec![
                        Cell::from(app.clock.format(r.timestamp, "%m/%d %H:%M:%S")),
                        Cell::from(r.provider.id()).style(style(pal.accent(r.provider))),
                        Cell::from(short_model(&r.model)),
                        Cell::from(grouped(r.output_tokens)),
                        Cell::from(
                            r.duration(app.args.max_gap)
                                .map_or_else(|| "—".into(), |d| format!("{d:.1}s")),
                        ),
                        Cell::from(rate(r.rate(app.args.max_gap)))
                            .style(bold(pal.accent(r.provider))),
                        Cell::from(r.session.chars().take(10).collect::<String>()),
                    ])
                });
            (
                vec![
                    "Completed",
                    "Source",
                    "Model",
                    "Output",
                    "Wait",
                    "Est. tok/s",
                    "Session",
                ],
                vec![
                    Constraint::Length(15),
                    Constraint::Length(8),
                    Constraint::Min(14),
                    count_column_width(
                        "Output",
                        app.view
                            .rows
                            .iter()
                            .rev()
                            .take(app.args.limit.max(200))
                            .map(|r| grouped(r.output_tokens)),
                    ),
                    Constraint::Length(8),
                    Constraint::Length(10),
                    Constraint::Length(11),
                ],
                rows.collect(),
            )
        };
    // A one-line header and its bottom margin precede the rows.
    const HEADER_LINES: u16 = 2;
    let header = Row::new(headers)
        .style(header_style)
        .bottom_margin(HEADER_LINES - 1);
    let block = pal.block(title);
    let inner = block.inner(area);
    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().bg(pal.selected).fg(pal.ink))
        .highlight_symbol("› ")
        .column_spacing(1);
    f.render_stateful_widget(table, area, &mut app.table);
    // One-line rows start below the header; `offset` is the first one drawn,
    // since the table scrolls to keep the selection visible.
    let body = Rect {
        y: inner.y + HEADER_LINES,
        height: inner.height.saturating_sub(HEADER_LINES),
        ..inner
    };
    let total = app.row_count();
    let first = app.table.offset();
    for i in 0..body.height as usize {
        if first + i >= total {
            break;
        }
        app.hits.push((
            Rect::new(body.x, body.y + i as u16, body.width, 1),
            Click::Row(first + i),
        ));
    }
}

pub(super) fn model_view(f: &mut Frame, area: Rect, app: &mut App) {
    if area.width < 105 {
        records_table(f, area, app);
        return;
    }
    let columns = Layout::horizontal([Constraint::Percentage(63), Constraint::Percentage(37)])
        .spacing(1)
        .split(area);
    let left = Layout::vertical([
        Constraint::Length((app.view.models.len() as u16 + 4).clamp(6, (area.height / 2).max(6))),
        Constraint::Min(5),
    ])
    .spacing(1)
    .split(columns[0]);
    records_table(f, left[0], app);
    context_plot(f, left[1], app);
    let side = Layout::vertical([Constraint::Length(10), Constraint::Min(5)]).split(columns[1]);
    model_inspector(f, side[0], app);
    distribution(f, side[1], app);
}

pub(super) fn model_inspector(f: &mut Frame, area: Rect, app: &App) {
    let pal = app.palette();
    let b = pal.block(" SELECTED MODEL ");
    let inner = b.inner(area);
    f.render_widget(b, area);
    let Some(m) = app.table.selected().and_then(|i| app.view.models.get(i)) else {
        return;
    };
    let color = pal.accent(m.provider);
    f.render_widget(
        Paragraph::new(vec![
            Line::styled(m.model.clone(), bold(color)),
            Line::raw(""),
            Line::styled(
                format!("{} weighted tok/s", rate(m.tokens_per_second)),
                bold(color),
            ),
            Line::styled(
                format!("{} median tok/s", rate(m.median_tokens_per_second)),
                style(pal.ink),
            ),
            Line::styled(
                format!(
                    "{} requests · {} output tokens",
                    m.requests,
                    compact(m.output_tokens)
                ),
                style(pal.muted),
            ),
            Line::styled(
                format!(
                    "{} / {} usable request intervals",
                    m.timed_requests, m.requests
                ),
                style(pal.muted),
            ),
            Line::styled("Request distribution below →", style(pal.muted)),
        ])
        .wrap(Wrap { trim: true }),
        inner,
    );
}

pub(super) fn distribution(f: &mut Frame, area: Rect, app: &App) {
    let pal = app.palette();
    let b = pal.block(" REQUEST SPEED DISTRIBUTION ");
    let inner = b.inner(area);
    f.render_widget(b, area);
    let Some(m) = app.table.selected().and_then(|i| app.view.models.get(i)) else {
        return;
    };
    let mut rates: Vec<f64> = app
        .view
        .rows
        .iter()
        .filter(|r| Some(r.provider) == m.provider && r.model == m.model)
        .filter_map(|r| r.rate(app.args.max_gap))
        .collect();
    rates.sort_by(f64::total_cmp);
    if rates.is_empty() {
        f.render_widget(
            Paragraph::new("No usable request timing").style(style(pal.muted)),
            inner,
        );
        return;
    }
    let color = pal.accent(m.provider);
    let labels = [
        (0.0, 10.0, "< 10"),
        (10.0, 25.0, "10–25"),
        (25.0, 50.0, "25–50"),
        (50.0, 100.0, "50–100"),
        (100.0, 200.0, "100–200"),
        (200.0, f64::INFINITY, ">= 200"),
    ];
    let counts: Vec<usize> = labels
        .iter()
        .map(|(lo, hi, _)| rates.iter().filter(|v| **v >= *lo && **v < *hi).count())
        .collect();
    let max = counts.iter().copied().max().unwrap_or(1).max(1);
    for (i, (_, _, label)) in labels
        .iter()
        .enumerate()
        .take(inner.height.saturating_sub(3) as usize)
    {
        let cols = Layout::horizontal([
            Constraint::Length(9),
            Constraint::Min(3),
            Constraint::Length(5),
        ])
        .split(Rect::new(inner.x, inner.y + i as u16, inner.width, 1));
        f.render_widget(Paragraph::new(*label).style(style(pal.muted)), cols[0]);
        f.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(color).bg(pal.edge))
                .ratio(counts[i] as f64 / max as f64)
                .label("")
                .use_unicode(true),
            cols[1],
        );
        f.render_widget(
            Paragraph::new(counts[i].to_string())
                .style(style(pal.ink))
                .alignment(Alignment::Right),
            cols[2],
        );
    }
    if inner.height >= 9 {
        let p90 = rates[((rates.len() - 1) as f64 * 0.9).ceil() as usize];
        f.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    format!("{} timed requests · speed buckets in tok/s", rates.len()),
                    style(pal.muted),
                ),
                Line::styled(
                    format!("P50 {}  ·  P90 {p90:.1}", rate(m.median_tokens_per_second)),
                    bold(color),
                ),
                Line::styled(
                    "Individual requests, not time-weighted bins.",
                    style(pal.muted),
                ),
            ])
            .wrap(Wrap { trim: true }),
            Rect::new(inner.x, inner.y + 7, inner.width, inner.height - 7),
        );
    }
}

pub(super) fn request_view(f: &mut Frame, area: Rect, app: &mut App) {
    let pal = app.palette();
    if area.width < 115 {
        records_table(f, area, app);
        return;
    }
    let columns = Layout::horizontal([Constraint::Percentage(69), Constraint::Percentage(31)])
        .spacing(1)
        .split(area);
    records_table(f, columns[0], app);
    let b = pal.block(" SELECTED REQUEST ");
    let inner = b.inner(columns[1]);
    f.render_widget(b, columns[1]);
    if let Some(r) = app
        .table
        .selected()
        .and_then(|i| app.view.rows.iter().rev().nth(i))
    {
        f.render_widget(
            Paragraph::new({
                let mut lines = record_detail(r, app);
                lines.pop();
                lines.push(Line::styled(
                    "↑/↓ select · Enter to inspect",
                    style(pal.muted),
                ));
                lines
            })
            .wrap(Wrap { trim: true }),
            inner,
        );
    }
}

pub(super) fn context_plot(f: &mut Frame, area: Rect, app: &App) {
    let pal = app.palette();
    let b = pal.block(" CONTEXT VS SPEED  ·  selected model ");
    let Some(m) = app.table.selected().and_then(|i| app.view.models.get(i)) else {
        f.render_widget(b, area);
        return;
    };
    let points: Vec<(f64, f64)> = app
        .view
        .rows
        .iter()
        .filter(|r| Some(r.provider) == m.provider && r.model == m.model)
        .filter_map(|r| {
            r.rate(app.args.max_gap)
                .map(|v| (r.input_tokens as f64 / 1000.0, v))
        })
        .collect();
    let x = (points.iter().map(|p| p.0).fold(0.0, f64::max) * 1.1).max(10.0);
    let y = ((points.iter().map(|p| p.1).fold(0.0, f64::max) * 1.1 / 25.0).ceil() * 25.0).max(50.0);
    let d = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Scatter)
        .style(style(pal.accent(m.provider)))
        .data(&points);
    let inner = b.inner(area);
    f.render_widget(b, area);
    let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(inner);
    f.render_widget(
        Chart::new(vec![d])
            .style(Style::default().bg(pal.panel))
            .x_axis(
                Axis::default()
                    .style(style(pal.edge))
                    .bounds([0.0, x])
                    .labels([
                        Line::styled("0", style(pal.muted)),
                        Line::styled(format!("{:.0}K", x / 2.0), style(pal.muted)),
                        Line::styled(format!("{x:.0}K"), style(pal.muted)),
                    ]),
            )
            .y_axis(
                Axis::default()
                    .style(style(pal.edge))
                    .bounds([0.0, y])
                    .labels([
                        Line::styled("0", style(pal.muted)),
                        Line::styled(format!("{:.0}", y / 2.0), style(pal.muted)),
                        Line::styled(format!("{y:.0}"), style(pal.muted)),
                    ]),
            ),
        rows[0],
    );
    f.render_widget(
        Paragraph::new("X: input tokens incl. cache · Y: request tok/s")
            .style(style(pal.muted))
            .alignment(Alignment::Center),
        rows[1],
    );
    if points.is_empty() {
        f.render_widget(
            Paragraph::new("No usable request timing")
                .style(style(pal.muted))
                .alignment(Alignment::Center),
            Rect::new(rows[0].x, rows[0].y + rows[0].height / 2, rows[0].width, 1),
        );
    }
}
