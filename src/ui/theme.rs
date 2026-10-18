//! Color themes. Every drawing function takes the active `Palette`, so a
//! theme switch is just a different value; nothing is global.

use crate::{args::Theme, source::Source};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding},
};

/// Colors for one theme. Accents in the light palette are darker so text
/// stays legible on white panels.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color,
    pub panel: Color,
    pub edge: Color,
    pub ink: Color,
    pub muted: Color,
    pub mint: Color,
    pub gold: Color,
    pub purple: Color,
    pub blue: Color,
    pub rose: Color,
    pub selected: Color,
    pub inspected: Color,
}

const DARK: Palette = Palette {
    bg: Color::Rgb(11, 16, 26),
    panel: Color::Rgb(17, 25, 39),
    edge: Color::Rgb(39, 52, 72),
    ink: Color::Rgb(220, 230, 244),
    muted: Color::Rgb(120, 139, 165),
    mint: Color::Rgb(102, 232, 191),
    gold: Color::Rgb(247, 183, 112),
    purple: Color::Rgb(178, 164, 255),
    blue: Color::Rgb(111, 185, 255),
    rose: Color::Rgb(255, 138, 178),
    selected: Color::Rgb(36, 47, 69),
    inspected: Color::Rgb(58, 65, 88),
};

const LIGHT: Palette = Palette {
    bg: Color::Rgb(247, 249, 252),
    panel: Color::Rgb(255, 255, 255),
    edge: Color::Rgb(174, 185, 201),
    ink: Color::Rgb(24, 36, 55),
    muted: Color::Rgb(79, 97, 119),
    mint: Color::Rgb(0, 113, 86),
    gold: Color::Rgb(145, 78, 0),
    purple: Color::Rgb(98, 65, 164),
    blue: Color::Rgb(29, 92, 171),
    rose: Color::Rgb(172, 40, 96),
    selected: Color::Rgb(225, 236, 249),
    inspected: Color::Rgb(218, 231, 249),
};

/// Background and text colors of a theme, for the SVG preview exporter.
#[cfg(test)]
pub fn base_colors(theme: Theme) -> (Color, Color) {
    let p = Palette::of(theme);
    (p.bg, p.ink)
}

pub fn style(c: Color) -> Style {
    Style::default().fg(c)
}

pub fn bold(c: Color) -> Style {
    style(c).add_modifier(Modifier::BOLD)
}

impl Palette {
    pub fn of(theme: Theme) -> Self {
        match theme {
            Theme::Dark => DARK,
            Theme::Light => LIGHT,
        }
    }
    /// Accent of a source; plain ink for an all-source total.
    pub fn accent(&self, s: impl Into<Option<Source>>) -> Color {
        match s.into() {
            Some(Source::Codex) => self.mint,
            Some(Source::Claude) => self.gold,
            Some(Source::Dsh) => self.rose,
            Some(Source::Other) => self.purple,
            None => self.ink,
        }
    }
    /// Display name and accent of a source.
    pub fn source(&self, s: Source) -> (&'static str, Color) {
        (s.label(), self.accent(s))
    }
    pub fn block(&self, title: impl Into<Line<'static>>) -> Block<'static> {
        Block::default()
            .title(title)
            .title_style(bold(self.ink))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(style(self.edge))
            .style(Style::default().bg(self.panel))
            .padding(Padding::horizontal(1))
    }
    pub fn chip(&self, label: String, active: bool, color: Color) -> Span<'static> {
        Span::styled(
            format!(" {label} "),
            if active {
                bold(self.bg).bg(color)
            } else {
                style(self.muted).bg(self.panel)
            },
        )
    }
    pub fn key(&self, k: &str, label: &str) -> Line<'static> {
        Line::from(vec![
            Span::styled(format!(" {k} "), bold(self.bg).bg(self.muted)),
            Span::styled(format!(" {label}  "), style(self.muted)),
        ])
    }
}
