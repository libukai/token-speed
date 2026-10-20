//! Timeline: throughput curves per time bucket and the bucket inspector.

use super::{
    overview::coverage_line,
    theme::{bold, style},
    widgets::*,
};
use crate::{
    app::{App, Click, Range, SOURCES},
    source::Source,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::Style,
    symbols,
    text::{Line, Span},
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph},
};

pub(super) fn kpis(f: &mut Frame, area: Rect, app: &App) {
    let pal = app.palette();
    let sources = app.visible_sources();
    let n = sources.len() as u32 + 2;
    let cols = Layout::horizontal((0..n).map(|_| Constraint::Ratio(1, n)))
        .spacing(1)
        .split(area);
    for (i, &p) in sources.iter().enumerate() {
        let (name, color) = pal.source(p);
        let s = &app.view.sources[p];
        Card {
            title: &name.to_uppercase(),
            value: rate(s.tokens_per_second),
            unit: "tok/s",
            note: format!("{} requests · weighted", s.requests),
            color,
        }
        .render(pal, f, cols[i]);
    }
    let cols = &cols[sources.len()..];
    let totals = &app.view.totals;
    Card {
        title: "OUTPUT",
        value: compact(totals.output_tokens),
        unit: "tokens",
        note: format!("{} total requests", totals.requests),
        color: pal.blue,
    }
    .render(pal, f, cols[0]);
    let percent = if totals.requests > 0 {
        totals.timed_requests as f64 / totals.requests as f64 * 100.0
    } else {
        0.0
    };
    Card {
        title: "TIMING",
        value: format!("{percent:.0}%"),
        unit: "usable",
        note: format!("{} / {} requests", totals.timed_requests, totals.requests),
        color: pal.purple,
    }
    .render(pal, f, cols[1]);
}

pub(super) fn trend(f: &mut Frame, area: Rect, app: &mut App, providers: &[Source]) {
    let pal = app.palette();
    let mut lines: [Vec<Vec<(f64, f64)>>; SOURCES] = Default::default();
    // Missing bins break the line. A gap is not zero throughput.
    for &p in providers {
        let segments = &mut lines[p];
        let mut current = Vec::new();
        for (i, values) in app.view.bins.iter().enumerate() {
            if let Some(v) = values[p] {
                current.push((i as f64, v));
            } else if !current.is_empty() {
                segments.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            segments.push(current);
        }
    }
    let mut datasets = Vec::new();
    for (p, segments) in Source::ALL.into_iter().zip(&lines) {
        let (name, color) = pal.source(p);
        for (i, points) in segments.iter().enumerate() {
            let mut d = Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(if points.len() > 1 {
                    GraphType::Line
                } else {
                    GraphType::Scatter
                })
                .style(style(color))
                .data(points);
            if i == 0 {
                d = d.name(name);
            }
            datasets.push(d);
        }
    }
    let max = app
        .view
        .bins
        .iter()
        .flatten()
        .flatten()
        .copied()
        .fold(0.0, f64::max);
    let upper = ((max * 1.15 / 25.0).ceil() * 25.0).max(50.0);
    let cursor_line = [(app.cursor as f64, 0.0), (app.cursor as f64, upper)];
    if !app.view.bins.is_empty() {
        datasets.insert(
            0,
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(style(pal.edge))
                .data(&cursor_line),
        );
    }
    let selected: Vec<Vec<(f64, f64)>> = providers
        .iter()
        .map(|p| {
            app.view
                .bins
                .get(app.cursor)
                .and_then(|v| v[*p])
                .map(|v| vec![(app.cursor as f64, v)])
                .unwrap_or_default()
        })
        .collect();
    for points in &selected {
        datasets.push(
            Dataset::default()
                .marker(symbols::Marker::Dot)
                .style(bold(pal.ink))
                .data(points),
        );
    }
    let xmax = (app.view.bins.len().saturating_sub(1) as f64).max(1.0);
    let time_fmt = if app.view.bucket_seconds >= 86400.0 {
        "%m/%d"
    } else if app.range == Range::Today {
        "%H:%M"
    } else {
        "%m/%d %Hh"
    };
    let labels = [
        app.view.chart_start,
        app.view.chart_start + xmax * 0.5 * app.view.bucket_seconds,
        app.view.chart_start + xmax * app.view.bucket_seconds,
    ]
    .map(|t| Line::styled(app.clock.format(t, time_fmt), style(pal.muted)));
    let title = Line::from(vec![
        Span::styled(
            match providers {
                [p] => format!(" {} ", pal.source(*p).0.to_uppercase()),
                _ => " THROUGHPUT ".into(),
            },
            bold(pal.ink),
        ),
        Span::styled(
            format!(
                " {} · weighted tok/s ",
                if app.view.bucket_seconds < 86400.0 {
                    format!("{}h bins", (app.view.bucket_seconds / 3600.0) as u64)
                } else {
                    format!("{}d bins", (app.view.bucket_seconds / 86400.0) as u64)
                }
            ),
            style(pal.muted),
        ),
    ]);
    let b = pal.block(title);
    let inside = b.inner(area);
    f.render_widget(b, area);
    let strip = if providers.len() == 1 {
        2
    } else {
        providers.len() as u16
    };
    let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(strip)]).split(inside);
    let chart = Chart::new(datasets)
        .style(Style::default().bg(pal.panel))
        .x_axis(
            Axis::default()
                .style(style(pal.edge))
                .bounds([0.0, xmax])
                .labels(labels),
        )
        .y_axis(
            Axis::default()
                .style(style(pal.edge))
                .bounds([0.0, upper])
                .labels([
                    Line::styled("0", style(pal.muted)),
                    Line::styled(format!("{:.0}", upper / 2.0), style(pal.muted)),
                    Line::styled(format!("{upper:.0}"), style(pal.muted)),
                ]),
        );
    f.render_widget(chart, rows[0]);
    let count = app.view.bins.len();
    if let Some(plot) = plot_area(f, rows[0])
        && count > 0
    {
        for dx in 0..plot.width {
            // Point i sits at i / (count - 1) of the width; pick the nearest.
            let i = ((dx as f64 + 0.5) / plot.width as f64 * xmax).round() as usize;
            app.hits.push((
                Rect::new(plot.x + dx, plot.y, 1, plot.height),
                Click::Bucket(i.min(count - 1)),
            ));
        }
    }
    if providers.len() == 1 {
        coverage_line(f, rows[1], app, providers[0], true);
    } else {
        for (i, p) in providers.iter().enumerate() {
            coverage_line(
                f,
                Rect::new(rows[1].x, rows[1].y + i as u16, rows[1].width, 1),
                app,
                *p,
                false,
            );
        }
    }
    if providers
        .iter()
        .all(|p| app.view.timed.iter().all(|v| v[*p] == 0))
    {
        let message = if app.loading && app.records.is_empty() {
            "Indexing your local logs…"
        } else if providers
            .iter()
            .all(|p| app.view.requests.iter().all(|r| r[*p] == 0))
        {
            "No requests · change the source or time window"
        } else {
            "Requests found, but no usable request timing"
        };
        let center = Rect::new(rows[0].x, rows[0].y + rows[0].height / 2, rows[0].width, 1);
        f.render_widget(
            Paragraph::new(message)
                .style(style(pal.muted))
                .alignment(Alignment::Center),
            center,
        );
    }
}

pub(super) fn timeline(f: &mut Frame, area: Rect, app: &mut App) {
    let pal = app.palette();
    let compact = area.height < 22;
    let visible = app.visible_sources();
    let inspector = visible.len() as u16 + if compact { 3 } else { 4 };
    let rows = Layout::vertical([
        Constraint::Length(if compact { 0 } else { 4 }),
        Constraint::Min(4),
        Constraint::Length(inspector),
    ])
    .split(area);
    if !compact {
        kpis(f, rows[0], app);
    }
    let providers = app.shown_sources();
    if compact || app.overlay || providers.len() == 1 {
        trend(f, rows[1], app, &providers);
    } else {
        let n = providers.len() as u32;
        let plots = Layout::vertical((0..n).map(|_| Constraint::Ratio(1, n)))
            .spacing(1)
            .split(rows[1]);
        for (i, &p) in providers.iter().enumerate() {
            trend(f, plots[i], app, &[p]);
        }
    }
    let b = pal.block(" BUCKET INSPECTOR  ·  ← / → to move  ·  g split / overlay ");
    let inner = b.inner(rows[2]);
    f.render_widget(b, rows[2]);
    f.render_widget(Paragraph::new(bucket_lines(app)), inner);
}

/// The plot inside a rendered chart: right of the y axis, above the x axis.
/// Read from the axis corner the chart drew, so label widths need not be
/// recomputed here.
fn plot_area(f: &mut Frame, chart: Rect) -> Option<Rect> {
    let buf = f.buffer_mut();
    (chart.top()..chart.bottom()).rev().find_map(|y| {
        let x = (chart.left()..chart.right()).find(|&x| buf[(x, y)].symbol() == "└")?;
        let x = x + 1;
        (x < chart.right() && y > chart.top())
            .then(|| Rect::new(x, chart.y, chart.right() - x, y - chart.y))
    })
}

pub(super) fn bucket_lines(app: &App) -> Vec<Line<'static>> {
    let pal = app.palette();
    if app.view.bins.is_empty() {
        return vec![Line::styled("No time buckets yet", style(pal.muted))];
    }
    let i = app.cursor.min(app.view.bins.len() - 1);
    let t = app.view.chart_start + i as f64 * app.view.bucket_seconds;
    let mut lines = vec![Line::styled(
        format!(
            "{}  →  {}",
            app.clock.format(t, "%m/%d %H:%M"),
            app.clock.format(t + app.view.bucket_seconds, "%m/%d %H:%M")
        ),
        bold(pal.ink),
    )];
    for p in app.visible_sources() {
        let (name, color) = pal.source(p);
        let note = if !app.args.provider.matches(p) {
            "Hidden by source filter".into()
        } else if app.view.requests[i][p] == 0 {
            "No requests recorded in this period".into()
        } else if app.view.timed[i][p] == 0 {
            format!(
                "{} requests · no usable timing · {} output",
                app.view.requests[i][p],
                compact(app.view.activity[i][p])
            )
        } else {
            format!(
                "{} tok/s · {} requests · {} timed · {} output",
                rate(app.view.bins[i][p]),
                app.view.requests[i][p],
                app.view.timed[i][p],
                compact(app.view.activity[i][p])
            )
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{name:<10}"), bold(color)),
            Span::styled(note, style(pal.muted)),
        ]));
    }
    lines
}
