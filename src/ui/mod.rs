//! Rendering. `draw` lays out the frame; each view lives in its own module.
//! Drawing also records click targets in `App::hits` for the next input.

mod dialogs;
mod overview;
mod tables;
mod theme;
mod timeline;
mod widgets;

pub use theme::Palette;
#[cfg(test)]
pub use theme::base_colors;
pub use widgets::{grouped, rate};

use crate::{
    app::{App, Click, Range},
    args::Screen,
    source::Source,
};
use dialogs::{detail, help, search};
use overview::overview;
use ratatui::widgets::Block;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use tables::{model_view, request_view};
use theme::{bold, style};
use timeline::timeline;
use widgets::*;

pub fn draw(f: &mut Frame, app: &mut App) {
    let pal = app.palette();
    let area = f.area();
    app.hits.clear();
    f.render_widget(
        Block::default().style(Style::default().bg(pal.bg).fg(pal.ink)),
        area,
    );
    if area.width < 60 || area.height < 20 {
        f.render_widget(Paragraph::new("TOKEN SPEED\n\nA little more room, please.\nMinimum 60 × 20 · Recommended 120 × 40\n\nResize the terminal or press q to exit.").style(style(pal.mint)).alignment(Alignment::Center).wrap(Wrap{trim:true}),area);
        return;
    }
    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(8),
        // Breathing room between the view and the key legend.
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
    let keyed = Source::ALL.iter().filter(|s| !s.spec().hidden).count();
    let sources = format!("1-{}", keyed + 1);
    let items = if area.width < 110 {
        vec![
            ("q", "quit"),
            ("Tab", "view"),
            ("d", "range"),
            ("/", "filter"),
            ("?", "help"),
        ]
    } else {
        let mut items = vec![
            ("Tab", "view"),
            (sources.as_str(), "source"),
            ("d", "range"),
        ];
        if app.screen == Screen::Timeline {
            items.extend([("←/→", "inspect"), ("g", "split/overlay")]);
        } else {
            items.extend([("↑/↓", "select"), ("Enter", "details")]);
        }
        items.extend([
            ("/", "filter"),
            ("T", "theme"),
            ("?", "help"),
            ("q", "quit"),
        ]);
        items
    };
    let spans = items
        .into_iter()
        .flat_map(|(k, l)| pal.key(k, l).spans)
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
    let pal = app.palette();
    let now = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
    let columns = Layout::horizontal([Constraint::Min(30), Constraint::Length(35)]).split(area);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(" ◈ ", bold(pal.mint)),
                Span::styled("TOKEN", bold(pal.ink)),
                Span::styled(" SPEED", bold(pal.mint)),
                Span::styled("  /  observatory", style(pal.muted)),
            ]),
            status_line(app, columns[0].width),
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
                Span::styled(state, bold(if app.paused { pal.gold } else { pal.mint })),
                Span::styled(
                    format!(
                        "  {} · v{}",
                        app.args.theme.label(),
                        env!("CARGO_PKG_VERSION")
                    ),
                    style(pal.muted),
                ),
            ]),
            Line::from(Span::styled(
                format!(
                    "{}  {}",
                    app.clock.format(now, "%b %d · %H:%M:%S"),
                    app.clock.label()
                ),
                style(pal.muted),
            )),
        ])
        .alignment(Alignment::Right),
        columns[1],
    );
}

fn toolbar(f: &mut Frame, area: Rect, app: &mut App) {
    let pal = app.palette();
    if area.width < 100 {
        let mut row = ChipRow::new();
        source_chips(app, &mut row);
        row.push(Span::styled("  WINDOW ", style(pal.muted)), None);
        row.push(
            pal.chip(app.range.label().into(), true, pal.purple),
            Some(Click::CycleRange),
        );
        row.push(Span::styled("  d to change", style(pal.muted)), None);
        row.render(f, area, false, app);
        return;
    }
    let halves =
        Layout::horizontal([Constraint::Percentage(46), Constraint::Percentage(54)]).split(area);
    let mut left = ChipRow::new();
    left.push(Span::styled(" SOURCE  ", style(pal.muted)), None);
    source_chips(app, &mut left);
    left.render(f, halves[0], false, app);
    let mut right = ChipRow::new();
    right.push(Span::styled(" WINDOW  ", style(pal.muted)), None);
    for r in [Range::Today, Range::Week, Range::Month, Range::All] {
        right.push(
            pal.chip(r.label().into(), r == app.range, pal.purple),
            Some(Click::Range(r)),
        );
        right.push(Span::raw(" "), None);
    }
    if app.range == Range::Custom {
        right.push(pal.chip("Custom".into(), true, pal.purple), None);
    }
    right.render(f, halves[1], true, app);
}

/// Under the title: what the dashboard is doing now, or what went wrong.
fn status_line(app: &App, width: u16) -> Line<'static> {
    let pal = app.palette();
    if let Some(e) = &app.error {
        Line::styled(format!(" ! {e}  ·  r to retry"), style(pal.gold))
    } else if app.loading {
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
            [((app.started.elapsed().as_millis() / 120) % 10) as usize];
        Line::from(vec![
            Span::styled(format!(" {spinner} "), style(pal.mint)),
            Span::styled(
                format!(
                    "Syncing · {} parsed / {} cached",
                    app.progress.0, app.progress.1
                ),
                style(pal.muted),
            ),
        ])
    } else {
        let age = app.refreshed.map_or(0, |t| t.elapsed().as_secs());
        let files = app.meta.parsed_files + app.meta.cached_files;
        let warnings = match app.meta.warnings.len() {
            0 => String::new(),
            n => format!(" · {n} warnings (? for details)"),
        };
        let filter = if app.search.is_empty() {
            String::new()
        } else {
            format!(" · filter: {}", app.search)
        };
        // Most to least informative: what is measured goes first when there
        // is room, and the freshness and warnings are what stay.
        let note = if width >= 100 {
            format!(
                "Estimated request throughput · gaps > {:.0}s excluded · {files} files · refreshed {age}s ago",
                app.args.max_gap
            )
        } else if width >= 62 {
            format!("Estimated throughput · {files} files · refreshed {age}s ago")
        } else {
            format!("refreshed {age}s ago")
        };
        Line::from(vec![
            Span::styled(" ● ", style(if app.paused { pal.gold } else { pal.mint })),
            Span::styled(format!("{note}{filter}{warnings}"), style(pal.muted)),
        ])
    }
}

fn navigation(f: &mut Frame, area: Rect, app: &mut App) {
    let pal = app.palette();
    let mut row = ChipRow::new();
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
        row.push(
            pal.chip(label, screen == app.screen, pal.purple),
            Some(Click::Screen(screen)),
        );
        row.push(Span::raw("  "), None);
    }
    let theme = format!("◐ {}", app.args.theme.label());
    let theme_width = theme.chars().count() as u16 + 2;
    let tabs = Rect::new(
        area.x,
        area.y,
        area.width.saturating_sub(theme_width + 1),
        area.height,
    );
    row.render(f, tabs, false, app);
    if area.width > row_min_for_theme(theme_width) {
        let mut toggle = ChipRow::new();
        toggle.push(pal.chip(theme, false, pal.purple), Some(Click::Theme));
        toggle.render(f, area, true, app);
    }
}

/// Hide the theme toggle on narrow terminals rather than overlap the tabs.
fn row_min_for_theme(theme_width: u16) -> u16 {
    52 + theme_width
}
