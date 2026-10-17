use crate::{
    app::{App, Range, SOURCES},
    args::{Provider, Screen},
    data::Record,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, BorderType, Borders, Cell, Chart, Clear, Dataset, Gauge, GraphType, Padding,
        Paragraph, Row, Sparkline, Table, Wrap,
    },
};

const BG: Color = Color::Rgb(11, 16, 26);
const PANEL: Color = Color::Rgb(17, 25, 39);
const EDGE: Color = Color::Rgb(39, 52, 72);
const INK: Color = Color::Rgb(220, 230, 244);
const MUTED: Color = Color::Rgb(120, 139, 165);
const MINT: Color = Color::Rgb(102, 232, 191);
const GOLD: Color = Color::Rgb(247, 183, 112);
const PURPLE: Color = Color::Rgb(178, 164, 255);
const BLUE: Color = Color::Rgb(111, 185, 255);
const ROSE: Color = Color::Rgb(255, 138, 178);
fn style(c: Color) -> Style {
    Style::default().fg(c)
}
fn bold(c: Color) -> Style {
    style(c).add_modifier(Modifier::BOLD)
}
fn block(title: impl Into<Line<'static>>) -> Block<'static> {
    Block::default()
        .title(title)
        .title_style(bold(INK))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style(EDGE))
        .style(Style::default().bg(PANEL))
        .padding(Padding::horizontal(1))
}
const SOURCE_COLORS: [Color; SOURCES] = [MINT, GOLD, ROSE];
fn accent(p: &str) -> Color {
    SOURCE_COLORS[Provider::slot(p)]
}
/// Display name and accent for a source slot.
fn source(p: usize) -> (&'static str, Color) {
    (Provider::SOURCES[p].label(), SOURCE_COLORS[p])
}
fn source_chips(app: &App) -> Vec<Span<'static>> {
    let mut spans = vec![chip(
        "1 All".into(),
        app.args.provider == Provider::All,
        PURPLE,
    )];
    for p in app.visible_sources() {
        let (name, color) = source(p);
        spans.push(Span::raw(" "));
        spans.push(chip(
            format!("{} {name}", p + 2),
            app.args.provider == Provider::SOURCES[p],
            color,
        ));
    }
    spans
}
pub fn compact(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}
pub fn rate(n: Option<f64>) -> String {
    n.map_or_else(|| "—".into(), |v| format!("{v:.1}"))
}
pub fn grouped(n: impl std::fmt::Display) -> String {
    let digits = n.to_string();
    let mut result = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(digit);
    }
    result
}
fn key(k: &str, label: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {k} "), bold(BG).bg(MUTED)),
        Span::styled(format!(" {label}  "), style(MUTED)),
    ])
}
fn chip(label: String, active: bool, color: Color) -> Span<'static> {
    Span::styled(
        format!(" {label} "),
        if active {
            bold(BG).bg(color)
        } else {
            style(MUTED).bg(PANEL)
        },
    )
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    app.view_tabs.clear();
    f.render_widget(
        Block::default().style(Style::default().bg(BG).fg(INK)),
        area,
    );
    if area.width < 60 || area.height < 20 {
        f.render_widget(Paragraph::new("TOKEN SPEED\n\nA little more room, please.\nMinimum 60 × 20 · Recommended 120 × 40\n\nResize the terminal or press q to exit.").style(style(MINT)).alignment(Alignment::Center).wrap(Wrap{trim:true}),area);
        return;
    }
    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(8),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .margin(1)
    .split(area);
    header(f, layout[0], app);
    navigation(f, layout[1], app);
    toolbar(f, layout[2], app);
    match app.screen {
        Screen::Overview => overview(f, layout[3], app),
        Screen::Timeline => timeline(f, layout[3], app),
        Screen::Models => model_view(f, layout[3], app),
        Screen::Requests => request_view(f, layout[3], app),
    }
    status(f, layout[4], app);
    let items = if area.width < 110 {
        vec![
            ("q", "quit"),
            ("Tab", "view"),
            ("d", "range"),
            ("/", "filter"),
            ("?", "help"),
        ]
    } else {
        let mut items = vec![("Tab", "view"), ("1/2/3", "source"), ("d", "range")];
        if app.screen == Screen::Timeline {
            items.extend([("←/→", "inspect"), ("g", "split/overlay")]);
        } else {
            items.extend([("↑/↓", "select"), ("Enter", "details")]);
        }
        items.extend([("/", "filter"), ("?", "help"), ("q", "quit")]);
        items
    };
    let spans = items
        .into_iter()
        .flat_map(|(k, l)| key(k, l).spans)
        .collect::<Vec<_>>();
    f.render_widget(Paragraph::new(Line::from(spans)), layout[5]);
    if app.help {
        help(f, app);
    } else if app.detail {
        detail(f, app);
    } else if app.editing {
        search(f, app);
    }
}
fn header(f: &mut Frame, area: Rect, app: &App) {
    let now = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
    let columns = Layout::horizontal([Constraint::Min(30), Constraint::Length(35)]).split(area);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(" ◈ ", bold(MINT)),
                Span::styled("TOKEN", bold(INK)),
                Span::styled(" SPEED", bold(MINT)),
                Span::styled("  /  observatory", style(MUTED)),
            ]),
            Line::from(Span::styled(
                "   Codex + Claude + DSH  ·  local token telemetry",
                style(MUTED),
            )),
        ]),
        columns[0],
    );
    let state = if app.paused {
        "Ⅱ PAUSED"
    } else if app.loading {
        "◌ SYNCING"
    } else {
        "● LIVE"
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(state, bold(if app.paused { GOLD } else { MINT })),
                Span::styled(
                    format!("  Rust · v{}", env!("CARGO_PKG_VERSION")),
                    style(MUTED),
                ),
            ]),
            Line::from(Span::styled(
                format!(
                    "{}  {}",
                    app.clock.format(now, "%b %d · %H:%M:%S"),
                    app.clock.label()
                ),
                style(MUTED),
            )),
        ])
        .alignment(Alignment::Right),
        columns[1],
    );
}
fn toolbar(f: &mut Frame, area: Rect, app: &App) {
    if area.width < 100 {
        f.render_widget(
            Paragraph::new(Line::from(
                source_chips(app)
                    .into_iter()
                    .chain([
                        Span::styled("  WINDOW ", style(MUTED)),
                        chip(app.range.label().into(), true, PURPLE),
                        Span::styled("  d to change", style(MUTED)),
                    ])
                    .collect::<Vec<_>>(),
            )),
            area,
        );
        return;
    }
    let halves =
        Layout::horizontal([Constraint::Percentage(46), Constraint::Percentage(54)]).split(area);
    let mut left = vec![Span::styled(" SOURCE  ", style(MUTED))];
    left.extend(source_chips(app));
    let left = Line::from(left);
    f.render_widget(Paragraph::new(left), halves[0]);
    let mut spans = vec![Span::styled(" WINDOW  ", style(MUTED))];
    for r in [Range::Today, Range::Week, Range::Month, Range::All] {
        spans.push(chip(r.label().into(), r == app.range, PURPLE));
        spans.push(Span::raw(" "));
    }
    if app.range == Range::Custom {
        spans.push(chip("Custom".into(), true, PURPLE));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Right),
        halves[1],
    );
}
fn card(
    f: &mut Frame,
    area: Rect,
    title: &str,
    value: String,
    unit: &str,
    note: String,
    color: Color,
) {
    let b = block(format!(" {title} ")).border_style(style(color));
    let inner = b.inner(area);
    f.render_widget(b, area);
    let mut lines = vec![Line::from(vec![
        Span::styled(value, bold(color)),
        Span::styled(format!("  {unit}"), style(MUTED)),
    ])];
    if area.height >= 5 {
        lines.push(Line::from(Span::styled(note, style(MUTED))));
    }
    f.render_widget(Paragraph::new(lines), inner);
}
fn kpis(f: &mut Frame, area: Rect, app: &App) {
    let sources = app.visible_sources();
    let n = sources.len() as u32 + 2;
    let cols = Layout::horizontal((0..n).map(|_| Constraint::Ratio(1, n)))
        .spacing(1)
        .split(area);
    for (i, &p) in sources.iter().enumerate() {
        let (name, color) = source(p);
        let s = &app.view.sources[p];
        card(
            f,
            cols[i],
            &name.to_uppercase(),
            rate(s.tokens_per_second),
            "tok/s",
            format!("{} requests · weighted", s.requests),
            color,
        );
    }
    let cols = &cols[sources.len()..];
    card(
        f,
        cols[0],
        "OUTPUT",
        compact(app.view.totals.output_tokens),
        "tokens",
        format!("{} total requests", app.view.totals.requests),
        BLUE,
    );
    let percent = if app.view.totals.requests > 0 {
        app.view.totals.timed_requests as f64 / app.view.totals.requests as f64 * 100.0
    } else {
        0.0
    };
    card(
        f,
        cols[1],
        "TIMING",
        format!("{percent:.0}%"),
        "usable",
        format!(
            "{} / {} requests",
            app.view.totals.timed_requests, app.view.totals.requests
        ),
        PURPLE,
    );
}
fn trend(f: &mut Frame, area: Rect, app: &App, providers: &[usize]) {
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
    for (p, segments) in lines.iter().enumerate() {
        let (name, color) = source(p);
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
                .style(style(EDGE))
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
                .style(bold(INK))
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
    .map(|t| Line::styled(app.clock.format(t, time_fmt), style(MUTED)));
    let title = Line::from(vec![
        Span::styled(
            match providers {
                [p] => format!(" {} ", source(*p).0.to_uppercase()),
                _ => " THROUGHPUT ".into(),
            },
            bold(INK),
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
            style(MUTED),
        ),
    ]);
    let b = block(title);
    let inside = b.inner(area);
    f.render_widget(b, area);
    let strip = if providers.len() == 1 {
        2
    } else {
        providers.len() as u16
    };
    let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(strip)]).split(inside);
    let chart = Chart::new(datasets)
        .style(Style::default().bg(PANEL))
        .x_axis(
            Axis::default()
                .style(style(EDGE))
                .bounds([0.0, xmax])
                .labels(labels),
        )
        .y_axis(
            Axis::default()
                .style(style(EDGE))
                .bounds([0.0, upper])
                .labels([
                    Line::styled("0", style(MUTED)),
                    Line::styled(format!("{:.0}", upper / 2.0), style(MUTED)),
                    Line::styled(format!("{upper:.0}"), style(MUTED)),
                ]),
        );
    f.render_widget(chart, rows[0]);
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
                .style(style(MUTED))
                .alignment(Alignment::Center),
            center,
        );
    }
}
fn leaderboard(f: &mut Frame, area: Rect, app: &App) {
    let b = block(" MODEL VELOCITY ");
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
    for (i, m) in models.iter().take((inner.height / 3) as usize).enumerate() {
        let y = inner.y + i as u16 * 3;
        let name = short_model(&m.model);
        let label = Line::from(vec![
            Span::styled(
                format!("{} ", m.provider.to_uppercase()),
                style(accent(&m.provider)),
            ),
            Span::styled(name, style(INK)),
        ]);
        let columns = Layout::horizontal([Constraint::Min(10), Constraint::Length(12)])
            .split(Rect::new(inner.x, y, inner.width, 1));
        f.render_widget(Paragraph::new(label), columns[0]);
        f.render_widget(
            Paragraph::new(format!("{} tok/s", rate(m.tokens_per_second)))
                .style(bold(accent(&m.provider)))
                .alignment(Alignment::Right),
            columns[1],
        );
        f.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(accent(&m.provider)).bg(EDGE))
                .ratio((m.tokens_per_second.unwrap_or(0.0) / max).clamp(0.0, 1.0))
                .label("")
                .use_unicode(true),
            Rect::new(inner.x, y + 1, inner.width, 1),
        );
    }
    if models.is_empty() {
        f.render_widget(
            Paragraph::new("Waiting for local telemetry…").style(style(MUTED)),
            inner,
        );
    }
}
fn activity(f: &mut Frame, area: Rect, app: &App) {
    let b = block(" OUTPUT ACTIVITY ");
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
        let (label, color) = source(p);
        let total = app.view.sources[p].output_tokens;
        if y + 1 >= inner.bottom() {
            break;
        }
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("{label}  "), style(color)),
                Span::styled(format!("{} tokens", compact(total)), style(MUTED)),
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
fn short_model(m: &str) -> String {
    m.strip_prefix("claude-").unwrap_or(m).to_owned()
}
fn count_column_width(header: &str, values: impl Iterator<Item = String>) -> Constraint {
    let width = values.map(|v| v.len()).max().unwrap_or(0).max(header.len());
    Constraint::Length(width as u16)
}
fn records_table(f: &mut Frame, area: Rect, app: &mut App) {
    let title = Line::from(vec![
        Span::styled(
            if app.screen == Screen::Requests {
                " REQUEST LOG "
            } else {
                " MODEL COMPARISON "
            },
            bold(INK),
        ),
        Span::styled("  ↑/↓ select · Enter to inspect ", style(MUTED)),
    ]);
    let header_style = style(MUTED).bg(PANEL);
    let (headers, widths, rows): (Vec<&str>, Vec<Constraint>, Vec<Row<'static>>) =
        if app.screen != Screen::Requests {
            let rows = app.view.models.iter().map(|m| {
                Row::new(vec![
                    Cell::from(m.provider.clone()).style(style(accent(&m.provider))),
                    Cell::from(short_model(&m.model)),
                    Cell::from(grouped(m.requests)),
                    Cell::from(grouped(m.output_tokens)),
                    Cell::from(rate(m.tokens_per_second)).style(bold(accent(&m.provider))),
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
                        Cell::from(r.provider.clone()).style(style(accent(&r.provider))),
                        Cell::from(short_model(&r.model)),
                        Cell::from(grouped(r.output_tokens)),
                        Cell::from(
                            r.duration(app.args.max_gap)
                                .map_or_else(|| "—".into(), |d| format!("{d:.1}s")),
                        ),
                        Cell::from(rate(r.rate(app.args.max_gap))).style(bold(accent(&r.provider))),
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
    let table = Table::new(rows, widths)
        .header(Row::new(headers).style(header_style).bottom_margin(1))
        .block(block(title))
        .row_highlight_style(Style::default().bg(Color::Rgb(36, 47, 69)).fg(INK))
        .highlight_symbol("› ")
        .column_spacing(1);
    f.render_stateful_widget(table, area, &mut app.table);
}
fn status(f: &mut Frame, area: Rect, app: &App) {
    let line = if let Some(e) = &app.error {
        Line::styled(format!(" ! {e}  ·  r to retry"), style(GOLD))
    } else if app.loading {
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
            [((app.started.elapsed().as_millis() / 120) % 10) as usize];
        Line::from(vec![
            Span::styled(format!(" {spinner} "), style(MINT)),
            Span::styled(
                format!(
                    "Syncing · {} parsed / {} cached",
                    app.progress.0, app.progress.1
                ),
                style(MUTED),
            ),
        ])
    } else {
        let age = app.refreshed.map_or(0, |t| t.elapsed().as_secs());
        let filter = if app.search.is_empty() {
            String::new()
        } else {
            format!(" · filter: {}", app.search)
        };
        Line::from(vec![
            Span::styled(" ● ", style(if app.paused { GOLD } else { MINT })),
            Span::styled(
                format!(
                    "Estimated request throughput · gaps > {:.0}s excluded · {} files · refreshed {age}s ago{}{}",
                    app.args.max_gap,
                    app.meta.parsed_files + app.meta.cached_files,
                    filter,
                    if app.meta.warnings.is_empty() {
                        String::new()
                    } else {
                        format!(" · {} warnings (? for details)", app.meta.warnings.len())
                    }
                ),
                style(MUTED),
            ),
        ])
    };
    f.render_widget(Paragraph::new(line), area);
}
fn popup(f: &mut Frame, width: u16, height: u16, title: &str, lines: Vec<Line<'static>>) {
    let a = f.area();
    let width = width.min(a.width.saturating_sub(4));
    let height = height.min(a.height.saturating_sub(2));
    let area = Rect::new(
        a.x + (a.width - width) / 2,
        a.y + (a.height - height) / 2,
        width,
        height,
    );
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lines)
            .block(
                block(format!(" {title} "))
                    .border_style(style(PURPLE))
                    .padding(Padding::new(2, 2, 1, 1)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}
fn help(f: &mut Frame, app: &App) {
    let mut lines = vec![
        Line::styled("YOUR TERMINAL, YOUR TELEMETRY", bold(MINT)),
        Line::raw(""),
        Line::raw("1 / 2 / 3 / 4 All / Codex / Claude / DSH"),
        Line::raw("d             Cycle the time window"),
        Line::raw("t / w / m / a Today / 7 days / 30 days / All time"),
        Line::raw("Tab / Shift-Tab Cycle views · F1–F4 direct selection"),
        Line::raw("Timeline: ← → inspect bucket · g split / overlay"),
        Line::raw("↑ ↓ or j k    Select a row · Enter opens details"),
        Line::raw("/             Filter model names and session IDs"),
        Line::raw("Esc           Clear filter / close dialog"),
        Line::raw("Space         Pause live updates"),
        Line::raw("r             Refresh · q or Ctrl-C quits"),
        Line::raw(""),
        Line::styled("HOW TO READ THE CHART", bold(PURPLE)),
        Line::raw("Output tokens ÷ estimated request seconds. Weighted by time."),
        Line::raw("Includes prefill, reasoning, network and residual waiting."),
        Line::raw("░ no requests · ! missing timing · colored bars = activity"),
        Line::raw("Missing intervals break the line; they are not zero speed."),
        Line::raw("Provider comparisons reflect different workloads."),
        Line::raw(""),
        Line::styled(
            format!("Local only · {} · no model API calls", app.clock.label()),
            style(MUTED),
        ),
    ];
    if let Some(w) = app.meta.warnings.first() {
        lines.push(Line::styled(format!("Warning: {w}"), style(GOLD)));
    }
    lines.push(Line::styled("\nPress Esc or Enter to return", style(MUTED)));
    popup(f, 74, 27, " KEYBOARD & METRICS ", lines);
}
fn detail(f: &mut Frame, app: &App) {
    if app.screen == Screen::Timeline {
        popup(f, 78, 14, " TIME BUCKET ", bucket_lines(app));
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
            Line::styled(m.model.clone(), bold(accent(&m.provider))),
            Line::raw(""),
            Line::raw(format!("Source         {}", m.provider)),
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
            Line::styled("Esc / Enter to return", style(MUTED)),
        ]
    } else {
        let Some(r) = app.view.rows.iter().rev().nth(i) else {
            return;
        };
        record_detail(r, app)
    };
    popup(f, 76, 21, " INSPECT ", lines);
}
fn record_detail(r: &Record, app: &App) -> Vec<Line<'static>> {
    vec![
        Line::styled(r.model.clone(), bold(accent(&r.provider))),
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
        Line::raw(format!("Session     {}", r.session)),
        Line::raw(format!("Response    {}", r.id)),
        Line::raw(""),
        Line::styled("Esc / Enter to return", style(MUTED)),
    ]
}
fn search(f: &mut Frame, app: &App) {
    popup(
        f,
        68,
        7,
        " FILTER ",
        vec![
            Line::styled(format!("› {}▏", app.search), bold(MINT)),
            Line::raw(""),
            Line::styled(
                "Model or session · Enter applies · Esc clears",
                style(MUTED),
            ),
        ],
    );
}

fn navigation(f: &mut Frame, area: Rect, app: &mut App) {
    let mut spans = Vec::new();
    let mut x = area.x;
    for screen in Screen::ALL {
        let label = format!(
            "F{} {}",
            screen.index() + 1,
            if area.width < 80 {
                ["Home", "Time", "Models", "Logs"][screen.index()]
            } else {
                screen.label()
            }
        );
        let width = (label.len() as u16 + 2).min(area.right().saturating_sub(x));
        if width > 0 {
            app.view_tabs.push((Rect::new(x, area.y, width, 1), screen));
        }
        x += label.len() as u16 + 4;
        spans.push(chip(label, screen == app.screen, PURPLE));
        spans.push(Span::raw("  "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn overview(f: &mut Frame, area: Rect, app: &App) {
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
        let right = Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(halves[1]);
        leaderboard(f, right[0], app);
        activity(f, right[1], app);
    } else {
        coverage_panel(f, rows[1], app);
    }
}

fn big_number(s: &str, color: Color) -> Vec<Line<'static>> {
    let font = [
        ["█▀█", "█ █", "▀▀▀"],
        [" ▀█", "  █", "  ▀"],
        ["▀▀█", "█▀▀", "▀▀▀"],
        ["▀▀█", " ▀█", "▀▀▀"],
        ["█ █", "▀▀█", "  ▀"],
        ["█▀▀", "▀▀█", "▀▀▀"],
        ["█▀▀", "█▀█", "▀▀▀"],
        ["▀▀█", "  █", "  ▀"],
        ["█▀█", "█▀█", "▀▀▀"],
        ["█▀█", "▀▀█", "▀▀▀"],
    ];
    (0..3)
        .map(|row| {
            Line::styled(
                s.chars()
                    .map(|c| match c {
                        '0'..='9' => font[c as usize - '0' as usize][row],
                        '.' => {
                            if row == 2 {
                                "▪"
                            } else {
                                " "
                            }
                        }
                        _ => {
                            if row == 1 {
                                "━━━"
                            } else {
                                "   "
                            }
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
                bold(color),
            )
        })
        .collect()
}

fn provider_card(f: &mut Frame, area: Rect, app: &App, p: usize) {
    let (name, color) = source(p);
    let s = &app.view.sources[p];
    let b = block(Line::from(vec![
        Span::styled(format!(" {} ", name.to_uppercase()), bold(color)),
        Span::styled(" weighted request throughput ", style(MUTED)),
    ]))
    .border_style(style(color));
    let inner = b.inner(area);
    f.render_widget(b, area);
    if !app.args.provider.matches(&name.to_lowercase()) {
        f.render_widget(
            Paragraph::new(vec![
                Line::styled("—", bold(color)),
                Line::raw(""),
                Line::styled("Hidden by source filter", style(MUTED)),
                Line::styled("Press 1 to compare all", style(MUTED)),
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
            style(MUTED),
        ));
    }
    lines.push(Line::from(vec![
        Span::styled(
            format!("Median {}", rate(s.median_tokens_per_second)),
            style(INK),
        ),
        Span::styled(format!("  ·  {} requests", s.requests), style(MUTED)),
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
        style(MUTED),
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

fn coverage_line(f: &mut Frame, area: Rect, app: &App, p: usize, legend: bool) {
    if area.height == 0 {
        return;
    }
    let (name, color) = source(p);
    let mut spans = vec![Span::styled(format!("{name:<7}"), style(color))];
    if !app.args.provider.matches(&name.to_lowercase()) {
        spans.push(Span::styled("hidden by source filter", style(MUTED)));
    } else if app.view.bins.is_empty() {
        spans.push(Span::styled("Waiting for local data…", style(MUTED)));
    } else {
        let cells = area.width.saturating_sub(7) as usize;
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
            let (symbol, c) = if app.view.requests[i][p] == 0 {
                ('░', MUTED)
            } else if app.view.timed[i][p] == 0 {
                ('!', GOLD)
            } else {
                (
                    shades[(app.view.activity[i][p] as f64 / max as f64 * 7.0).round() as usize],
                    color,
                )
            };
            let bg = if app.screen == Screen::Timeline && i == app.cursor {
                Color::Rgb(58, 65, 88)
            } else {
                PANEL
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
                Span::styled("       ░ no requests", style(MUTED)),
                Span::styled("  ! no usable timing", style(GOLD)),
                Span::styled("  ▁..█ output volume", style(color)),
            ])),
            Rect::new(area.x, area.y + 1, area.width, 1),
        );
    }
}

fn coverage_panel(f: &mut Frame, area: Rect, app: &App) {
    let b = block(" ACTIVITY MAP  ·  activity by time bucket ");
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
                Paragraph::new("░ no requests · ! no timing").style(style(MUTED)),
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
        Paragraph::new(format!("{start}  →  {end}")).style(style(MUTED)),
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
                    Span::styled("░ no requests  ", style(MUTED)),
                    Span::styled("! no usable timing  ", style(GOLD)),
                    Span::styled("▁..█ output volume", style(MINT)),
                ]),
                Line::styled(
                    "A quiet period is missing activity, not a measured speed of zero.",
                    style(MUTED),
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

fn overview_note(f: &mut Frame, area: Rect, app: &App) {
    let b = block(" LATEST ACTIVITY ");
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
        .style(style(MUTED)),
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
                style(MUTED),
            ),
            Span::styled(format!("  {:<6}", r.provider), style(accent(&r.provider))),
            Span::styled(
                format!("  {} tok/s", rate(r.rate(app.args.max_gap))),
                bold(accent(&r.provider)),
            ),
            Span::styled(
                format!(
                    "  {} output  ·  {}",
                    compact(r.output_tokens),
                    short_model(&r.model)
                ),
                style(MUTED),
            ),
        ]);
        f.render_widget(
            Paragraph::new(line),
            Rect::new(inner.x, inner.y + 2 + i as u16, inner.width, 1),
        );
    }
}

fn timeline(f: &mut Frame, area: Rect, app: &App) {
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
    let providers: Vec<usize> = match app.args.provider {
        Provider::All => visible,
        p => vec![Provider::slot(p.label())],
    };
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
    let b = block(" BUCKET INSPECTOR  ·  ← / → to move  ·  g split / overlay ");
    let inner = b.inner(rows[2]);
    f.render_widget(b, rows[2]);
    f.render_widget(Paragraph::new(bucket_lines(app)), inner);
}

fn bucket_lines(app: &App) -> Vec<Line<'static>> {
    if app.view.bins.is_empty() {
        return vec![Line::styled("No time buckets yet", style(MUTED))];
    }
    let i = app.cursor.min(app.view.bins.len() - 1);
    let t = app.view.chart_start + i as f64 * app.view.bucket_seconds;
    let mut lines = vec![Line::styled(
        format!(
            "{}  →  {}",
            app.clock.format(t, "%m/%d %H:%M"),
            app.clock.format(t + app.view.bucket_seconds, "%m/%d %H:%M")
        ),
        bold(INK),
    )];
    for p in app.visible_sources() {
        let (name, color) = source(p);
        let note = if !app.args.provider.matches(&name.to_lowercase()) {
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
            Span::styled(format!("{name:<8}"), bold(color)),
            Span::styled(note, style(MUTED)),
        ]));
    }
    lines
}

fn model_view(f: &mut Frame, area: Rect, app: &mut App) {
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

fn model_inspector(f: &mut Frame, area: Rect, app: &App) {
    let b = block(" SELECTED MODEL ");
    let inner = b.inner(area);
    f.render_widget(b, area);
    let Some(m) = app.table.selected().and_then(|i| app.view.models.get(i)) else {
        return;
    };
    let color = accent(&m.provider);
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
                style(INK),
            ),
            Line::styled(
                format!(
                    "{} requests · {} output tokens",
                    m.requests,
                    compact(m.output_tokens)
                ),
                style(MUTED),
            ),
            Line::styled(
                format!(
                    "{} / {} usable request intervals",
                    m.timed_requests, m.requests
                ),
                style(MUTED),
            ),
            Line::styled("Request distribution below →", style(MUTED)),
        ])
        .wrap(Wrap { trim: true }),
        inner,
    );
}

fn distribution(f: &mut Frame, area: Rect, app: &App) {
    let b = block(" REQUEST SPEED DISTRIBUTION ");
    let inner = b.inner(area);
    f.render_widget(b, area);
    let Some(m) = app.table.selected().and_then(|i| app.view.models.get(i)) else {
        return;
    };
    let mut rates: Vec<f64> = app
        .view
        .rows
        .iter()
        .filter(|r| r.provider == m.provider && r.model == m.model)
        .filter_map(|r| r.rate(app.args.max_gap))
        .collect();
    rates.sort_by(f64::total_cmp);
    if rates.is_empty() {
        f.render_widget(
            Paragraph::new("No usable request timing").style(style(MUTED)),
            inner,
        );
        return;
    }
    let color = accent(&m.provider);
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
        f.render_widget(Paragraph::new(*label).style(style(MUTED)), cols[0]);
        f.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(color).bg(EDGE))
                .ratio(counts[i] as f64 / max as f64)
                .label("")
                .use_unicode(true),
            cols[1],
        );
        f.render_widget(
            Paragraph::new(counts[i].to_string())
                .style(style(INK))
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
                    style(MUTED),
                ),
                Line::styled(
                    format!("P50 {}  ·  P90 {p90:.1}", rate(m.median_tokens_per_second)),
                    bold(color),
                ),
                Line::styled("Individual requests, not time-weighted bins.", style(MUTED)),
            ])
            .wrap(Wrap { trim: true }),
            Rect::new(inner.x, inner.y + 7, inner.width, inner.height - 7),
        );
    }
}

fn request_view(f: &mut Frame, area: Rect, app: &mut App) {
    if area.width < 115 {
        records_table(f, area, app);
        return;
    }
    let columns = Layout::horizontal([Constraint::Percentage(69), Constraint::Percentage(31)])
        .spacing(1)
        .split(area);
    records_table(f, columns[0], app);
    let b = block(" SELECTED REQUEST ");
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
                lines.push(Line::styled("↑/↓ select · Enter to inspect", style(MUTED)));
                lines
            })
            .wrap(Wrap { trim: true }),
            inner,
        );
    }
}

fn context_plot(f: &mut Frame, area: Rect, app: &App) {
    let b = block(" CONTEXT VS SPEED  ·  selected model ");
    let Some(m) = app.table.selected().and_then(|i| app.view.models.get(i)) else {
        f.render_widget(b, area);
        return;
    };
    let points: Vec<(f64, f64)> = app
        .view
        .rows
        .iter()
        .filter(|r| r.provider == m.provider && r.model == m.model)
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
        .style(style(accent(&m.provider)))
        .data(&points);
    let inner = b.inner(area);
    f.render_widget(b, area);
    let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(inner);
    f.render_widget(
        Chart::new(vec![d])
            .style(Style::default().bg(PANEL))
            .x_axis(Axis::default().style(style(EDGE)).bounds([0.0, x]).labels([
                Line::styled("0", style(MUTED)),
                Line::styled(format!("{:.0}K", x / 2.0), style(MUTED)),
                Line::styled(format!("{x:.0}K"), style(MUTED)),
            ]))
            .y_axis(Axis::default().style(style(EDGE)).bounds([0.0, y]).labels([
                Line::styled("0", style(MUTED)),
                Line::styled(format!("{:.0}", y / 2.0), style(MUTED)),
                Line::styled(format!("{y:.0}"), style(MUTED)),
            ])),
        rows[0],
    );
    f.render_widget(
        Paragraph::new("X: input tokens incl. cache · Y: request tok/s")
            .style(style(MUTED))
            .alignment(Alignment::Center),
        rows[1],
    );
    if points.is_empty() {
        f.render_widget(
            Paragraph::new("No usable request timing")
                .style(style(MUTED))
                .alignment(Alignment::Center),
            Rect::new(rows[0].x, rows[0].y + rows[0].height / 2, rows[0].width, 1),
        );
    }
}
