use crate::{
    args::{Args, Group, Screen},
    data::{self, Clock, Metadata, Record, Summary},
    source::{Filter, Source},
};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Rect, widgets::TableState};
use std::{
    io::Stdout,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

pub const SOURCES: usize = Source::COUNT;

/// Something on screen that reacts to a left click.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Click {
    Screen(Screen),
    Filter(Filter),
    Range(Range),
    CycleRange,
    Theme,
    Row(usize),
    Bucket(usize),
}

/// Source filter for a digit key: 1 is all, then one per source.
fn digit_filter(c: char) -> Option<Filter> {
    Filter::from_digit(c.to_digit(10)?)
}

pub enum Load {
    Progress(usize, usize),
    Done(Result<(Vec<Record>, Metadata)>),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Range {
    Today,
    Week,
    Month,
    All,
    Custom,
}
impl Range {
    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Week => "7 days",
            Self::Month => "30 days",
            Self::All => "All time",
            Self::Custom => "Custom",
        }
    }
}
pub struct View {
    pub rows: Vec<Record>,
    pub models: Vec<Summary>,
    pub totals: Summary,
    /// Per-source values below are indexed by `Source::index`.
    pub sources: [Summary; SOURCES],
    pub bins: Vec<[Option<f64>; SOURCES]>,
    pub activity: Vec<[u64; SOURCES]>,
    pub requests: Vec<[usize; SOURCES]>,
    pub timed: Vec<[usize; SOURCES]>,
    pub chart_start: f64,
    pub bucket_seconds: f64,
    /// Sources with any indexed record, regardless of the current filters.
    pub indexed: [bool; SOURCES],
}
impl Default for View {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            models: Vec::new(),
            totals: Summary::default(),
            sources: Default::default(),
            bins: Vec::new(),
            activity: Vec::new(),
            requests: Vec::new(),
            timed: Vec::new(),
            chart_start: 0.0,
            bucket_seconds: 3600.0,
            indexed: [false; SOURCES],
        }
    }
}
pub struct App {
    pub args: Args,
    pub clock: Clock,
    pub range: Range,
    pub records: Vec<Record>,
    pub view: View,
    pub meta: Metadata,
    pub loading: bool,
    pub paused: bool,
    pub progress: (usize, usize),
    pub refreshed: Option<Instant>,
    pub started: Instant,
    pub error: Option<String>,
    pub help: bool,
    pub detail: bool,
    pub screen: Screen,
    pub cursor: usize,
    pub overlay: bool,
    /// Click targets recorded by the most recent draw.
    pub hits: Vec<(Rect, Click)>,
    pub table: TableState,
    pub search: String,
    pub editing: bool,
    tx: Sender<Load>,
    rx: Receiver<Load>,
}
impl App {
    pub fn new(args: Args, clock: Clock) -> Self {
        let range = if args.today {
            Range::Today
        } else if args.all_time {
            Range::All
        } else if args.since.is_some() || args.until.is_some() {
            Range::Custom
        } else if args.days == 7 {
            Range::Week
        } else if args.days == 30 {
            Range::Month
        } else {
            Range::Custom
        };
        let screen = args.view;
        let (tx, rx) = mpsc::channel();
        Self {
            args,
            clock,
            range,
            records: Vec::new(),
            view: View::default(),
            meta: Metadata::default(),
            loading: false,
            paused: false,
            progress: (0, 0),
            refreshed: None,
            started: Instant::now(),
            error: None,
            help: false,
            detail: false,
            screen,
            cursor: usize::MAX,
            overlay: false,
            hits: Vec::new(),
            table: TableState::default().with_selected(Some(0)),
            search: String::new(),
            editing: false,
            tx,
            rx,
        }
    }
    pub fn reload(&mut self) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.error = None;
        self.progress = (0, 0);
        let mut args = self.args.clone();
        args.provider = Filter::ALL;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let progress_tx = tx.clone();
            let result = data::collect(&args, |p, c| {
                let _ = progress_tx.send(Load::Progress(p, c));
            });
            let _ = tx.send(Load::Done(result));
        });
        self.args.refresh = false;
    }
    pub fn poll(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Load::Progress(p, c) => self.progress = (p, c),
                Load::Done(result) => {
                    self.loading = false;
                    self.refreshed = Some(Instant::now());
                    match result {
                        Ok((rows, meta)) => {
                            self.records = rows;
                            self.meta = meta;
                            if let Err(e) = self.rebuild() {
                                self.error = Some(e.to_string());
                            }
                        }
                        Err(e) => self.error = Some(format!("{e:#}")),
                    }
                }
            }
        }
        if !self.loading
            && !self.paused
            && self
                .refreshed
                .is_some_and(|t| t.elapsed().as_secs_f64() >= self.args.interval)
        {
            self.reload();
        }
    }
    pub fn set_range(&mut self, range: Range) -> Result<()> {
        self.range = range;
        self.cursor = usize::MAX;
        self.args.since = None;
        self.args.until = None;
        self.args.today = range == Range::Today;
        self.args.all_time = range == Range::All;
        self.args.days = if range == Range::Month { 30 } else { 7 };
        self.table.select(Some(0));
        self.rebuild()
    }
    pub fn rebuild(&mut self) -> Result<()> {
        let mut rows = data::select(&self.records, &self.args, self.clock)?;
        if !self.search.is_empty() {
            let query = self.search.to_lowercase();
            rows.retain(|r| {
                r.model.to_lowercase().contains(&query)
                    || r.session.to_lowercase().contains(&query)
                    || r.speed_mode.as_deref().is_some_and(|m| m.contains(&query))
            });
        }
        let totals = data::summarize(&rows, self.args.max_gap);
        let sources = Source::ALL
            .map(|s| data::summarize(rows.iter().filter(|r| r.provider == s), self.args.max_gap));
        let mut models = data::aggregate(&rows, Group::Model, self.clock, self.args.max_gap);
        models.sort_by(|a, b| b.output_tokens.cmp(&a.output_tokens));
        let (since, until) = data::bounds(&self.args, self.clock)?;
        let now = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
        let start = since.unwrap_or_else(|| rows.first().map_or(now - 86400.0, |r| r.timestamp));
        let end = until.unwrap_or(now).max(start + 1.0);
        let span = end - start;
        let step = if span <= 86400.0 {
            3600.0
        } else if span <= 8.0 * 86400.0 {
            6.0 * 3600.0
        } else if span <= 32.0 * 86400.0 {
            86400.0
        } else {
            (span / 48.0).ceil().max(86400.0)
        };
        let count = (span / step).ceil().clamp(1.0, 96.0) as usize;
        let mut tokens = vec![[0u64; SOURCES]; count];
        let mut seconds = vec![[0.0; SOURCES]; count];
        let mut activity = vec![[0u64; SOURCES]; count];
        let mut requests = vec![[0usize; SOURCES]; count];
        let mut timed = vec![[0usize; SOURCES]; count];
        for r in &rows {
            let bin = (((r.timestamp - start) / step).floor().max(0.0) as usize).min(count - 1);
            let p = r.provider.index();
            activity[bin][p] += r.output_tokens;
            requests[bin][p] += 1;
            if let Some(d) = r.duration(self.args.max_gap) {
                timed[bin][p] += 1;
                tokens[bin][p] += r.output_tokens;
                seconds[bin][p] += d;
            }
        }
        let bins = (0..count)
            .map(|i| {
                std::array::from_fn(|p| {
                    (seconds[i][p] > 0.0).then(|| tokens[i][p] as f64 / seconds[i][p])
                })
            })
            .collect();
        let mut indexed = [false; SOURCES];
        for r in &self.records {
            indexed[r.provider.index()] = true;
        }
        self.view = View {
            rows,
            models,
            totals,
            sources,
            bins,
            activity,
            requests,
            timed,
            chart_start: start,
            bucket_seconds: step,
            indexed,
        };
        self.cursor = self.cursor.min(count.saturating_sub(1));
        let count = self.row_count();
        self.table.select(if count > 0 {
            Some(self.table.selected().unwrap_or(0).min(count - 1))
        } else {
            None
        });
        Ok(())
    }
    /// Sources to display. Optional sources appear once their logs are
    /// indexed or when selected explicitly.
    pub fn visible_sources(&self) -> Vec<Source> {
        Source::ALL
            .into_iter()
            .filter(|&s| {
                !s.spec().optional
                    || self.args.provider == Filter::only(s)
                    || self.view.indexed[s.index()]
            })
            .collect()
    }
    /// Sources shown in the timeline: the selected one, or all visible.
    pub fn shown_sources(&self) -> Vec<Source> {
        match self.args.provider.0 {
            Some(s) => vec![s],
            None => self.visible_sources(),
        }
    }
    pub fn palette(&self) -> crate::ui::Palette {
        crate::ui::Palette::of(self.args.theme)
    }
    pub fn row_count(&self) -> usize {
        if self.screen == Screen::Requests {
            self.view.rows.len().min(self.args.limit.max(200))
        } else {
            self.view.models.len()
        }
    }
    pub fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
        self.table = TableState::default().with_selected((self.row_count() > 0).then_some(0));
    }
    /// Handle a left click against the targets of the last frame.
    pub fn click(&mut self, column: u16, row: u16) -> Result<()> {
        // A click outside a dialog dismisses it rather than being lost.
        if self.help || self.detail {
            self.help = false;
            self.detail = false;
            return Ok(());
        }
        // Keep the typed filter, as Enter would, then act on the click.
        self.editing = false;
        let Some(target) = self
            .hits
            .iter()
            .rev()
            .find(|(r, _)| column >= r.x && column < r.right() && row >= r.y && row < r.bottom())
            .map(|(_, c)| *c)
        else {
            return Ok(());
        };
        match target {
            Click::Screen(screen) => self.set_screen(screen),
            Click::Filter(filter) => {
                self.args.provider = filter;
                self.table.select(Some(0));
                self.rebuild()?;
            }
            Click::Range(range) => self.set_range(range)?,
            Click::CycleRange => self.cycle_range()?,
            Click::Theme => self.args.theme = self.args.theme.toggled(),
            Click::Row(i) => {
                if self.table.selected() == Some(i) {
                    self.detail = true;
                } else {
                    self.table.select(Some(i));
                }
            }
            Click::Bucket(i) => self.cursor = i,
        }
        Ok(())
    }
    fn cycle_range(&mut self) -> Result<()> {
        self.set_range(match self.range {
            Range::Today => Range::Week,
            Range::Week => Range::Month,
            Range::Month => Range::All,
            _ => Range::Today,
        })
    }
    pub fn key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.kind == KeyEventKind::Release {
            return Ok(false);
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(true);
        }
        if self.editing {
            match key.code {
                KeyCode::Esc => {
                    self.editing = false;
                    self.search.clear();
                }
                KeyCode::Enter => self.editing = false,
                KeyCode::Backspace => {
                    self.search.pop();
                }
                KeyCode::Char(c) => self.search.push(c),
                _ => {}
            }
            self.table.select(Some(0));
            self.rebuild()?;
            return Ok(false);
        }
        if self.help || self.detail {
            match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?') => {
                    self.help = false;
                    self.detail = false;
                }
                KeyCode::Char('q') => return Ok(true),
                _ => {}
            }
            return Ok(false);
        }
        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char(c) if digit_filter(c).is_some() => {
                self.args.provider = digit_filter(c).unwrap();
                self.table.select(Some(0));
                self.rebuild()?;
            }
            KeyCode::Char('d') => self.cycle_range()?,
            KeyCode::Char('T') => self.args.theme = self.args.theme.toggled(),
            KeyCode::Char('t') => {
                self.set_range(Range::Today)?;
            }
            KeyCode::Char('w') => {
                self.set_range(Range::Week)?;
            }
            KeyCode::Char('m') => {
                self.set_range(Range::Month)?;
            }
            KeyCode::Char('a') => {
                self.set_range(Range::All)?;
            }
            KeyCode::Char(' ') => self.paused = !self.paused,
            KeyCode::Char('r') => self.reload(),
            KeyCode::Char('g') if self.screen == Screen::Timeline => self.overlay = !self.overlay,
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('/') => self.editing = true,
            KeyCode::Esc => {
                self.search.clear();
                self.args.model = None;
                self.args.session = None;
            }
            KeyCode::Tab | KeyCode::Char('v') => {
                self.set_screen(Screen::ALL[(self.screen.index() + 1) % 4])
            }
            KeyCode::BackTab => self.set_screen(Screen::ALL[(self.screen.index() + 3) % 4]),
            KeyCode::F(n @ 1..=4) => self.set_screen(Screen::ALL[(n - 1) as usize]),
            KeyCode::Left if self.screen == Screen::Timeline => {
                self.cursor = self.cursor.saturating_sub(1)
            }
            KeyCode::Right if self.screen == Screen::Timeline => {
                self.cursor = (self.cursor + 1).min(self.view.bins.len().saturating_sub(1))
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let n = self.row_count();
                if n > 0 {
                    self.table
                        .select(Some((self.table.selected().unwrap_or(0) + 1).min(n - 1)));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.table
                    .select(Some(self.table.selected().unwrap_or(0).saturating_sub(1)));
            }
            KeyCode::Home => self.table.select(Some(0)),
            KeyCode::End => {
                let n = self.row_count();
                self.table.select(n.checked_sub(1));
            }
            KeyCode::Enter => {
                if self.screen == Screen::Timeline
                    || (self.screen != Screen::Overview && self.table.selected().is_some())
                {
                    self.detail = true;
                }
            }
            _ => {}
        }
        if key.code == KeyCode::Esc {
            self.table.select(Some(0));
            self.rebuild()?;
        }
        Ok(false)
    }
}
pub fn run(
    args: Args,
    clock: Clock,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    shutdown: &AtomicBool,
) -> Result<()> {
    let mut app = App::new(args, clock);
    app.reload();
    while !shutdown.load(Ordering::Relaxed) {
        app.poll();
        terminal.draw(|f| crate::ui::draw(f, &mut app))?;
        if !event::poll(Duration::from_millis(150))? {
            continue;
        }
        // Skip events that change nothing without redrawing, so a burst of
        // motion or release reports cannot delay the next click. Stop at the
        // first event that does change state: the next click must be tested
        // against a freshly drawn frame.
        loop {
            match app.event(event::read()?)? {
                Handled::Quit => return Ok(()),
                Handled::Changed => break,
                Handled::Ignored if event::poll(Duration::ZERO)? => continue,
                Handled::Ignored => break,
            }
        }
    }
    Ok(())
}

pub enum Handled {
    Quit,
    Changed,
    Ignored,
}
impl App {
    pub fn event(&mut self, event: Event) -> Result<Handled> {
        use crossterm::event::{MouseButton, MouseEventKind};
        let code = match event {
            Event::Key(key) => {
                return Ok(if self.key(key)? {
                    Handled::Quit
                } else {
                    Handled::Changed
                });
            }
            Event::Resize(..) => return Ok(Handled::Changed),
            Event::Mouse(m) => match m.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    self.click(m.column, m.row)?;
                    return Ok(Handled::Changed);
                }
                MouseEventKind::ScrollDown if self.screen == Screen::Timeline => KeyCode::Right,
                MouseEventKind::ScrollDown => KeyCode::Down,
                MouseEventKind::ScrollUp if self.screen == Screen::Timeline => KeyCode::Left,
                MouseEventKind::ScrollUp => KeyCode::Up,
                _ => return Ok(Handled::Ignored),
            },
            _ => return Ok(Handled::Ignored),
        };
        self.key(KeyEvent::new(code, KeyModifiers::NONE))?;
        Ok(Handled::Changed)
    }
}
