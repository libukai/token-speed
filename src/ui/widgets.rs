//! Shared widgets and number formatting.

use super::theme::{Palette, bold, style};
use crate::{
    app::{App, Click},
    source::Filter,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::Color,
    text::{Line, Span},
    widgets::{Clear, Padding, Paragraph, Wrap},
};

/// Lays out spans left to right from `x`, recording a click target for each
/// span that has one. Width is measured in terminal cells, not bytes.
pub(super) struct ChipRow {
    spans: Vec<Span<'static>>,
    targets: Vec<(u16, u16, Click)>,
    width: u16,
}

impl ChipRow {
    pub(super) fn new() -> Self {
        Self {
            spans: Vec::new(),
            targets: Vec::new(),
            width: 0,
        }
    }
    pub(super) fn push(&mut self, span: Span<'static>, click: Option<Click>) {
        let w = span.width() as u16;
        if let Some(c) = click {
            self.targets.push((self.width, w, c));
        }
        self.width = self.width.saturating_add(w);
        self.spans.push(span);
    }
    /// Render in `area`, aligned left or right, and keep the visible targets.
    pub(super) fn render(self, f: &mut Frame, area: Rect, right: bool, app: &mut App) {
        let x0 = if right {
            area.x + area.width.saturating_sub(self.width)
        } else {
            area.x
        };
        for (dx, w, c) in self.targets {
            let x = x0.saturating_add(dx);
            let w = w.min(area.right().saturating_sub(x));
            if w > 0 {
                app.hits.push((Rect::new(x, area.y, w, 1), c));
            }
        }
        let p = Paragraph::new(Line::from(self.spans));
        f.render_widget(
            if right {
                p.alignment(Alignment::Right)
            } else {
                p
            },
            Rect::new(area.x, area.y, area.width, area.height.min(1)),
        );
    }
}

pub(super) fn source_chips(app: &App, row: &mut ChipRow) {
    let pal = app.palette();
    let filters =
        std::iter::once(Filter::ALL).chain(app.visible_sources().into_iter().map(Filter::only));
    for (i, filter) in filters.enumerate() {
        if i > 0 {
            row.push(Span::raw(" "), None);
        }
        let color = filter.0.map_or(pal.purple, |s| pal.accent(s));
        row.push(
            pal.chip(
                format!("{} {}", filter.digit(), filter.label()),
                app.args.provider == filter,
                color,
            ),
            Some(Click::Filter(filter)),
        );
    }
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

/// A small KPI box: a headline value with its unit, and a note below.
pub(super) struct Card<'a> {
    pub title: &'a str,
    pub value: String,
    pub unit: &'a str,
    pub note: String,
    pub color: Color,
}
impl Card<'_> {
    pub(super) fn render(self, pal: Palette, f: &mut Frame, area: Rect) {
        let b = pal
            .block(format!(" {} ", self.title))
            .border_style(style(self.color));
        let inner = b.inner(area);
        f.render_widget(b, area);
        let mut lines = vec![Line::from(vec![
            Span::styled(self.value, bold(self.color)),
            Span::styled(format!("  {}", self.unit), style(pal.muted)),
        ])];
        if area.height >= 5 {
            lines.push(Line::from(Span::styled(self.note, style(pal.muted))));
        }
        f.render_widget(Paragraph::new(lines), inner);
    }
}

pub(super) fn short_model(m: &str) -> String {
    m.strip_prefix("claude-").unwrap_or(m).to_owned()
}

pub(super) fn count_column_width(header: &str, values: impl Iterator<Item = String>) -> Constraint {
    let width = values.map(|v| v.len()).max().unwrap_or(0).max(header.len());
    Constraint::Length(width as u16)
}

pub(super) fn popup(
    pal: Palette,
    f: &mut Frame,
    width: u16,
    height: u16,
    title: &str,
    lines: Vec<Line<'static>>,
) {
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
                pal.block(format!(" {title} "))
                    .border_style(style(pal.purple))
                    .padding(Padding::new(2, 2, 1, 1)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

pub(super) fn big_number(s: &str, color: Color) -> Vec<Line<'static>> {
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
