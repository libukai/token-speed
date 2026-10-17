use crate::source::{Filter, Source};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Command {
    #[default]
    Tui,
    Summary,
    Recent,
    Watch,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Group {
    Hour,
    #[default]
    Day,
    Model,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Format {
    #[default]
    Table,
    Json,
    Csv,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}
impl Theme {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Screen {
    #[default]
    Overview,
    Timeline,
    Models,
    Requests,
}
impl Screen {
    pub const ALL: [Self; 4] = [Self::Overview, Self::Timeline, Self::Models, Self::Requests];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap()
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Timeline => "Timeline",
            Self::Models => "Models",
            Self::Requests => "Requests",
        }
    }
}

#[derive(Parser, Clone, Debug)]
#[command(
    version,
    about = "Local token throughput observatory for coding agents (Codex, Claude Code, DSH)",
    after_help = "Run token-speed with no arguments for the TUI, --table for a text report, or --json for JSON. Use summary/recent for scriptable reports."
)]
pub struct Args {
    #[arg(value_enum, default_value = "tui")]
    pub command: Command,
    #[arg(
        long,
        value_enum,
        default_value = "overview",
        help = "Initial TUI view"
    )]
    pub view: Screen,
    #[arg(
        long,
        value_enum,
        default_value = "dark",
        env = "TOKEN_SPEED_THEME",
        help = "TUI color theme; T toggles it while running"
    )]
    pub theme: Theme,
    #[arg(long, value_enum, default_value = "all", help = "Log source")]
    pub provider: Filter,
    #[arg(long, value_enum, default_value = "day")]
    pub group: Group,
    #[arg(long, conflicts_with_all=["days", "all_time", "since"])]
    pub today: bool,
    #[arg(long, default_value_t=7, conflicts_with_all=["today", "all_time", "since"])]
    pub days: u32,
    #[arg(long, conflicts_with_all=["today", "days", "since"])]
    pub all_time: bool,
    #[arg(long, conflicts_with_all=["today", "days", "all_time"], help="Inclusive ISO date/time")]
    pub since: Option<String>,
    #[arg(long, help = "Exclusive ISO time, or inclusive calendar date")]
    pub until: Option<String>,
    #[arg(long, help = "IANA timezone, e.g. Asia/Taipei")]
    pub timezone: Option<String>,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long)]
    pub session: Option<String>,
    #[arg(
        long,
        value_enum,
        help = "Report format (default: table); bypasses the TUI"
    )]
    pub format: Option<Format>,
    #[arg(long, conflicts_with_all = ["json", "format"], help = "Print a bordered text table instead of opening the TUI")]
    pub table: bool,
    #[arg(long, conflicts_with_all = ["table", "format"], help = "Print JSON instead of opening the TUI")]
    pub json: bool,
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    #[arg(long, default_value_t = 300.0)]
    pub max_gap: f64,
    #[arg(long, default_value_t = 5.0)]
    pub interval: f64,
    #[arg(long, default_value_os_t = Source::Codex.default_dir(), help = "Codex home (CODEX_HOME)")]
    pub codex_dir: PathBuf,
    #[arg(long, default_value_os_t = Source::Claude.default_dir(), help = "Claude Code home (CLAUDE_CONFIG_DIR)")]
    pub claude_dir: PathBuf,
    #[arg(long, default_value_os_t = Source::Dsh.default_dir(), help = "DSH home (DSH_HOME)")]
    pub dsh_dir: PathBuf,
    #[arg(long, default_value_os_t=cache_path())]
    pub cache: PathBuf,
    #[arg(long)]
    pub refresh: bool,
}
impl Args {
    pub fn output_format(&self) -> Format {
        if self.json {
            Format::Json
        } else {
            self.format.unwrap_or_default()
        }
    }

    /// Home directory of a source.
    pub fn dir(&self, source: Source) -> &PathBuf {
        match source {
            Source::Codex => &self.codex_dir,
            Source::Claude => &self.claude_dir,
            Source::Dsh => &self.dsh_dir,
        }
    }

    pub fn uses_tui(&self) -> bool {
        matches!(self.command, Command::Tui | Command::Watch)
            && self.format.is_none()
            && !self.table
            && !self.json
    }
}
pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
fn cache_path() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".cache"))
        .join("token-speed/index.sqlite3")
}
