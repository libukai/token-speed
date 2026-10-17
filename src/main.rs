mod app;
mod args;
mod data;
mod report;
#[cfg(test)]
mod tests;
mod ui;

use anyhow::{Result, bail};
use args::{Args, Command};
use clap::Parser;
use data::Clock;
use std::io::{self, IsTerminal};
use std::sync::{Arc, atomic::AtomicBool};

fn main() {
    if let Err(e) = entry() {
        if e.downcast_ref::<io::Error>()
            .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe)
        {
            return;
        }
        eprintln!("token-speed: {e:#}");
        std::process::exit(1);
    }
}
fn entry() -> Result<()> {
    let args = Args::parse();
    if args.days == 0
        || args.limit == 0
        || !args.max_gap.is_finite()
        || args.max_gap <= 0.0
        || !args.interval.is_finite()
        || args.interval <= 0.0
    {
        bail!("days, limit, max-gap and interval must be finite and positive");
    }
    let clock = Clock::new(
        args.timezone
            .as_deref()
            .or(std::env::var("TZ").ok().as_deref()),
    )?;
    data::bounds(&args, clock)?;
    if args.uses_tui() && io::stdout().is_terminal() && io::stdin().is_terminal() {
        let shutdown = Arc::new(AtomicBool::new(false));
        #[cfg(unix)]
        for signal in [
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGHUP,
        ] {
            signal_hook::flag::register(signal, Arc::clone(&shutdown))?;
        }
        struct Restore;
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableMouseCapture);
                ratatui::restore();
            }
        }
        let mut terminal = ratatui::init();
        let _restore = Restore;
        // Report presses, releases and the wheel only. Crossterm's
        // EnableMouseCapture also turns on any-motion tracking (1003), which
        // floods the loop with events while the pointer moves.
        #[cfg(unix)]
        {
            use std::io::Write;
            let mut out = io::stdout();
            out.write_all(b"\x1b[?1000h\x1b[?1015h\x1b[?1006h")?;
            out.flush()?;
        }
        #[cfg(not(unix))]
        crossterm::execute!(io::stdout(), crossterm::event::EnableMouseCapture)?;
        return app::run(args, clock, &mut terminal, &shutdown);
    }
    if args.command == Command::Watch {
        bail!("watch requires an interactive terminal; use summary --format json for scripts");
    }
    report::run(&args, clock)
}
