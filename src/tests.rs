use super::*;
use chrono::{TimeZone, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use data::Record;
use ratatui::{Terminal, backend::TestBackend, style::Color};
use serde_json::Value;
use serde_json::json;
use std::time::Instant;
use std::{fs, io::Write};

#[test]
fn report_flags_choose_output_without_opening_tui() {
    use args::Format;
    assert!(Args::parse_from(["token-speed"]).uses_tui());
    assert!(Args::parse_from(["token-speed", "watch"]).uses_tui());
    for (flags, format) in [
        (vec!["--table"], Format::Table),
        (vec!["--json"], Format::Json),
        (vec!["--format", "table"], Format::Table),
        (vec!["--format", "json"], Format::Json),
        (vec!["--format", "csv"], Format::Csv),
        (vec!["summary"], Format::Table),
        (vec!["recent", "--json"], Format::Json),
    ] {
        let args = Args::parse_from(std::iter::once("token-speed").chain(flags));
        assert!(!args.uses_tui());
        assert_eq!(args.output_format(), format);
    }
    for flags in [
        vec!["--table", "--json"],
        vec!["--table", "--format", "json"],
        vec!["--json", "--format", "table"],
    ] {
        assert!(Args::try_parse_from(std::iter::once("token-speed").chain(flags)).is_err());
    }
}

fn report_fixture() -> Vec<Record> {
    let base = Utc
        .with_ymd_and_hms(2026, 10, 8, 0, 0, 0)
        .unwrap()
        .timestamp() as f64;
    [
        ("fast", 100, Some(10.0)),
        ("slow", 200, Some(40.0)),
        ("模型\n測試\u{1b}", 500, None),
        ("excluded", 600, Some(400.0)),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (model, output_tokens, seconds))| Record {
        provider: "codex".into(),
        session: "synthetic-session".into(),
        id: format!("r{i}"),
        model: model.into(),
        timestamp: base + i as f64,
        start: seconds.map(|s| base + i as f64 - s),
        input_tokens: 1000,
        cached_tokens: 800,
        cache_write_tokens: 0,
        output_tokens,
        reasoning_tokens: 50,
        timing_source: "synthetic".into(),
    })
    .collect()
}

fn render_report(args: &Args, rows: &[Record]) -> String {
    let mut out = Vec::new();
    report::write_report(
        &mut out,
        args,
        Clock::new(Some("UTC")).unwrap(),
        rows,
        &data::Metadata::default(),
    )
    .unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn bordered_table_aligns_unicode_and_preserves_weighted_totals() {
    let args = Args::parse_from(["token-speed", "--table"]);
    let text = render_report(&args, &report_fixture());
    let table: Vec<_> = text
        .lines()
        .filter(|line| line.starts_with(['┌', '├', '│', '└']))
        .collect();
    let width = ratatui::text::Line::from(table[0]).width();
    assert!(
        table
            .iter()
            .all(|line| ratatui::text::Line::from(*line).width() == width)
    );
    assert!(table[0].ends_with('┐'));
    assert!(table.last().unwrap().ends_with('┘'));
    assert!(text.contains("模型 測試 "));
    assert!(!text.contains('\u{1b}'));
    assert_eq!(text.matches('—').count(), 4);
    let cells: Vec<_> = table
        .iter()
        .find(|line| line.contains("Total"))
        .unwrap()
        .split('│')
        .skip(1)
        .take(10)
        .map(str::trim)
        .collect();
    assert_eq!(
        cells,
        [
            "Total", "", "", "4", "2/4", "4,000", "3,200", "1,400", "6.0", "7.5"
        ]
    );
    let fast = table.iter().find(|line| line.contains("fast")).unwrap();
    assert!(fast.contains("│   1 │"));
}

#[test]
fn json_reports_keep_null_timing_and_recent_limits() {
    let rows = report_fixture();
    let args = Args::parse_from(["token-speed", "recent", "--json"]);
    let value: Value = serde_json::from_str(&render_report(&args, &rows)).unwrap();
    assert_eq!(
        value["metric"],
        "estimated_request_output_tokens_per_second"
    );
    assert_eq!(value["timezone"], "UTC");
    let records = value["data"].as_array().unwrap();
    assert_eq!(records.len(), 4);
    assert_eq!(records[0]["timing_status"], "estimated");
    assert_eq!(records[0]["tokens_per_second"], 10.0);
    assert_eq!(records[2]["timing_status"], "missing");
    assert_eq!(records[3]["timing_status"], "excluded");
    for r in &records[2..] {
        assert!(r["seconds"].is_null());
        assert!(r["tokens_per_second"].is_null());
    }
    let args = Args::parse_from(["token-speed", "recent", "--json", "--limit", "2"]);
    let value: Value = serde_json::from_str(&render_report(&args, &rows)).unwrap();
    assert_eq!(value["data"].as_array().unwrap().len(), 2);
    assert_eq!(value["data"][0]["id"], "r2");
    let args = Args::parse_from(["token-speed", "--json", "--group", "model"]);
    let value: Value = serde_json::from_str(&render_report(&args, &rows)).unwrap();
    let summaries = value["data"].as_array().unwrap();
    assert_eq!(
        summaries
            .iter()
            .map(|s| s["output_tokens"].as_u64().unwrap())
            .sum::<u64>(),
        1400
    );
    let missing = summaries
        .iter()
        .find(|s| s["model"] == rows[2].model)
        .unwrap();
    assert_eq!(missing["output_tokens"], 500);
    assert_eq!(missing["timed_requests"], 0);
    assert!(missing["tokens_per_second"].is_null());
}

#[test]
fn empty_and_recent_text_reports_are_noninteractive() {
    let args = Args::parse_from(["token-speed", "--table"]);
    assert!(render_report(&args, &[]).contains("No matching token records."));
    let args = Args::parse_from(["token-speed", "--json"]);
    let value: Value = serde_json::from_str(&render_report(&args, &[])).unwrap();
    assert_eq!(value["data"], json!([]));
    let args = Args::parse_from(["token-speed", "recent", "--table", "--limit", "1"]);
    let text = render_report(&args, &report_fixture());
    assert!(text.contains("excluded"));
    assert!(!text.contains("fast"));
    assert!(!text.contains("Total"));
    assert!(text.contains('—'));
}

fn fixture(records: Vec<Value>, provider: &str) -> Vec<Record> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.jsonl");
    fs::write(
        &path,
        records
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    data::parse_file(&path, provider).unwrap().0
}
fn codex(t: u32, typ: &str, p: Value) -> Value {
    json!({"timestamp":format!("2026-10-08T00:00:{t:02}Z"),"type":typ,"payload":p})
}
#[test]
fn codex_mirrors_and_tools() {
    let rows = fixture(
        vec![
            codex(0, "session_meta", json!({"id":"s"})),
            codex(0, "turn_context", json!({"model":"gpt-test"})),
            codex(0, "event_msg", json!({"type":"task_started"})),
            codex(
                10,
                "token_usage_record",
                json!({"thread_id":"s","response_id":"r1","usage":{"output_tokens":100}}),
            ),
            codex(
                11,
                "event_msg",
                json!({"type":"token_count","info":{"total_token_usage":{"output_tokens":100},"last_token_usage":{"output_tokens":100}}}),
            ),
            codex(30, "response_item", json!({"type":"function_call_output"})),
            codex(
                40,
                "token_usage_record",
                json!({"thread_id":"s","response_id":"r2","usage":{"output_tokens":200}}),
            ),
            codex(
                41,
                "token_usage_record",
                json!({"thread_id":"child","response_id":"r3","usage":{"output_tokens":999}}),
            ),
        ],
        "codex",
    );
    assert_eq!(
        rows.iter().map(|r| r.output_tokens).collect::<Vec<_>>(),
        vec![100, 200]
    );
    assert_eq!(
        rows.iter().map(|r| r.duration(300.0)).collect::<Vec<_>>(),
        vec![Some(10.0), Some(10.0)]
    );
}
#[test]
fn legacy_duplicate_and_reset() {
    let usage = |t, total, last| {
        codex(
            t,
            "event_msg",
            json!({"type":"token_count","info":{"total_token_usage":{"output_tokens":total},"last_token_usage":{"output_tokens":last}}}),
        )
    };
    let rows = fixture(
        vec![
            usage(1, 100, 100),
            usage(2, 100, 100),
            usage(3, 150, 50),
            usage(4, 20, 20),
        ],
        "codex",
    );
    assert_eq!(
        rows.iter().map(|r| r.output_tokens).collect::<Vec<_>>(),
        vec![100, 50, 20]
    );
    assert!(rows[0].start.is_none());
}
#[test]
fn claude_blocks_branch_and_cache() {
    let user = |id: &str, t, parent: Option<&str>| json!({"type":"user","uuid":id,"parentUuid":parent,"timestamp":format!("2026-10-08T00:00:{t:02}Z"),"sessionId":"s"});
    let assistant = |id: &str, mid: &str, t, parent: &str, output| json!({"type":"assistant","uuid":id,"parentUuid":parent,"sessionId":"s","timestamp":format!("2026-10-08T00:00:{t:02}Z"),"message":{"id":mid,"model":"claude-test","usage":{"input_tokens":2,"cache_read_input_tokens":30,"cache_creation_input_tokens":8,"output_tokens":output}}});
    let rows = fixture(
        vec![
            user("u", 0, None),
            assistant("a", "m", 5, "u", 100),
            assistant("b", "m", 10, "a", 120),
            assistant("x", "other", 12, "u", 100),
            user("v", 20, Some("b")),
            assistant("c", "n", 30, "v", 100),
        ],
        "claude",
    );
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].output_tokens, 120);
    assert_eq!(rows[0].input_tokens, 40);
    assert_eq!(
        rows.iter().map(|r| r.duration(300.0)).collect::<Vec<_>>(),
        vec![Some(10.0), Some(12.0), Some(10.0)]
    );
}
#[test]
fn weighted_speed_and_exclusions() {
    let rows = fixture(
        vec![
            codex(0, "event_msg", json!({"type":"task_started"})),
            codex(
                10,
                "token_usage_record",
                json!({"response_id":"a","usage":{"output_tokens":100}}),
            ),
            codex(
                30,
                "token_usage_record",
                json!({"response_id":"b","usage":{"output_tokens":100}}),
            ),
        ],
        "codex",
    );
    let summary = data::summarize(&rows, 300.0);
    assert!((summary.tokens_per_second.unwrap() - 200.0 / 30.0).abs() < 1e-6);
    assert_eq!(summary.median_tokens_per_second, Some(7.5));
    let short = data::summarize(&rows, 15.0);
    assert_eq!(short.timed_requests, 1);
    assert_eq!(short.output_tokens, 200);
    assert_eq!(short.tokens_per_second, Some(10.0));
}
#[test]
fn cache_append_dedup_and_deleted_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("sessions")).unwrap();
    let path = dir.path().join("sessions/a.jsonl");
    let records = [
        codex(0, "session_meta", json!({"id":"s"})),
        codex(0, "event_msg", json!({"type":"task_started"})),
        codex(
            10,
            "token_usage_record",
            json!({"response_id":"a","usage":{"output_tokens":100}}),
        ),
    ];
    fs::write(
        &path,
        records
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
    let mut args = Args::parse_from(["token-speed", "--provider", "codex"]);
    args.codex_dir = dir.path().into();
    args.cache = dir.path().join("index.sqlite3");
    let (rows, meta) = data::collect(&args, |_, _| {}).unwrap();
    assert_eq!(meta.parsed_files, 1);
    assert_eq!(rows.len(), 1);
    assert_eq!(data::collect(&args, |_, _| {}).unwrap().1.cached_files, 1);
    writeln!(
        fs::OpenOptions::new().append(true).open(&path).unwrap(),
        "{}",
        codex(
            20,
            "token_usage_record",
            json!({"response_id":"b","usage":{"output_tokens":100}})
        )
    )
    .unwrap();
    let (rows, meta) = data::collect(&args, |_, _| {}).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(meta.parsed_files, 1);
    fs::copy(&path, dir.path().join("sessions/b.jsonl")).unwrap();
    assert_eq!(data::collect(&args, |_, _| {}).unwrap().0.len(), 2);
    fs::remove_file(path).unwrap();
    fs::remove_file(dir.path().join("sessions/b.jsonl")).unwrap();
    assert!(data::collect(&args, |_, _| {}).unwrap().0.is_empty());
}
#[test]
fn dates_timezone_and_partial_lines() {
    let args = Args::parse_from([
        "token-speed",
        "--since",
        "2026-10-08",
        "--until",
        "2026-10-08",
        "--timezone",
        "Asia/Taipei",
    ]);
    let clock = Clock::new(args.timezone.as_deref()).unwrap();
    let (start, end) = data::bounds(&args, clock).unwrap();
    assert_eq!(end.unwrap() - start.unwrap(), 86400.0);
    assert_eq!(
        clock.format(start.unwrap(), "%Y-%m-%d %H:%M"),
        "2026-10-08 00:00"
    );
    assert!(data::stamp(&json!("bad")).is_none());
    assert_eq!(
        data::stamp(&json!(1791417600000i64)),
        data::stamp(&json!("2026-10-08T00:00:00Z"))
    );
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("test.jsonl");
    fs::write(&p, "[]\n{broken\n").unwrap();
    let (rows, bad) = data::parse_file(&p, "codex").unwrap();
    assert!(rows.is_empty());
    assert_eq!(bad, 2);
}
fn demo_app() -> app::App {
    let mut args = Args::parse_from([
        "token-speed",
        "--since",
        "2026-10-08",
        "--until",
        "2026-10-08",
        "--timezone",
        "UTC",
    ]);
    args.interval = 600.0;
    let clock = Clock::new(Some("UTC")).unwrap();
    let mut app = app::App::new(args, clock);
    let base = Utc
        .with_ymd_and_hms(2026, 10, 8, 0, 0, 0)
        .unwrap()
        .timestamp() as f64;
    for hour in 0..24 {
        for provider in ["codex", "claude"] {
            if hour == 9 && provider == "codex" {
                continue;
            }
            for request in 0..8 {
                let speed = if provider == "codex" {
                    38.0 + (hour as f64 * 0.7).sin() * 12.0
                } else {
                    84.0 + (hour as f64 * 0.5).sin() * 22.0
                };
                let end = base + hour as f64 * 3600.0 + request as f64 * 180.0 + 50.0;
                app.records.push(Record {
                    provider: provider.into(),
                    session: format!("demo-{provider}"),
                    id: format!("r-{hour}-{request}"),
                    model: if provider == "codex" {
                        "gpt-6.1-sol"
                    } else {
                        "claude-opus-5-5"
                    }
                    .into(),
                    timestamp: end,
                    start: Some(end - 12.0),
                    input_tokens: 45000 + hour as u64 * 2000 + request as u64 * 3000,
                    cached_tokens: 42000,
                    cache_write_tokens: 0,
                    output_tokens: (speed * 12.0) as u64,
                    reasoning_tokens: 80,
                    timing_source: "request-boundary".into(),
                });
            }
        }
    }
    app.records
        .sort_by(|a, b| a.timestamp.total_cmp(&b.timestamp));
    app.meta.cached_files = 8194;
    app.refreshed = Some(Instant::now());
    app.rebuild().unwrap();
    app
}
fn press(app: &mut app::App, code: KeyCode) {
    app.key(KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
}
#[test]
fn tui_filters_pause_navigation_and_chart_gaps() {
    let mut app = demo_app();
    assert_eq!(app.view.bins.len(), 24);
    assert!(app.view.bins[9][0].is_none());
    assert!(app.view.bins[9][1].is_some());
    press(&mut app, KeyCode::Char('2'));
    assert!(app.view.rows.iter().all(|r| r.provider == "codex"));
    assert_eq!(app.view.sources[1].requests, 0);
    press(&mut app, KeyCode::Char(' '));
    assert!(app.paused);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.screen, args::Screen::Timeline);
    press(&mut app, KeyCode::F(4));
    assert_eq!(app.screen, args::Screen::Requests);
    press(&mut app, KeyCode::Down);
    assert_eq!(app.table.selected(), Some(1));
    press(&mut app, KeyCode::Enter);
    assert!(app.detail);
    press(&mut app, KeyCode::Esc);
    assert!(!app.detail);
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('z'));
    assert!(app.view.rows.is_empty());
    press(&mut app, KeyCode::Esc);
    assert!(!app.view.rows.is_empty());
    press(&mut app, KeyCode::Char('?'));
    assert!(app.help);
    press(&mut app, KeyCode::Esc);
    assert!(!app.help);
    press(&mut app, KeyCode::Char('t'));
    assert_eq!(app.range, app::Range::Today);
}
#[test]
fn tui_tables_show_grouped_counts_and_missing_timing_in_split_views() {
    let mut app = demo_app();
    app.records.truncate(1);
    app.records[0].output_tokens = 1_234_567;
    app.records[0].start = None;
    app.rebuild().unwrap();
    for screen in [args::Screen::Models, args::Screen::Requests] {
        app.set_screen(screen);
        for width in [120, 140] {
            let mut term = Terminal::new(TestBackend::new(width, 40)).unwrap();
            term.draw(|f| ui::draw(f, &mut app)).unwrap();
            let buffer = term.backend().buffer();
            let row = (0..40)
                .map(|y| {
                    (0..width)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .find(|line| line.contains("1,234,567"))
                .unwrap_or_else(|| panic!("grouped output missing in {screen:?} at width {width}"));
            assert!(
                row.contains('—'),
                "missing timing hidden in {screen:?} at width {width}"
            );
            if screen == args::Screen::Models {
                assert!(row.contains("0/1"), "timed count hidden at width {width}");
            }
        }
    }
}

#[test]
fn tui_render_sizes_empty_loading_and_modals() {
    let mut app = demo_app();
    for screen in args::Screen::ALL {
        app.set_screen(screen);
        for (width, height) in [
            (140, 46),
            (120, 40),
            (100, 30),
            (80, 24),
            (60, 20),
            (40, 12),
        ] {
            let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
            term.draw(|f| ui::draw(f, &mut app)).unwrap();
            let text = term
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(text.contains("TOKEN"));
            if width >= 105 {
                let expected = match screen {
                    args::Screen::Overview => "ACTIVITY MAP",
                    args::Screen::Timeline => "BUCKET INSPECTOR",
                    args::Screen::Models => "MODEL COMPARISON",
                    args::Screen::Requests => "REQUEST LOG",
                };
                assert!(
                    text.contains(expected),
                    "missing {expected} at {width}x{height}"
                );
            }
        }
    }
    let mut term = Terminal::new(TestBackend::new(140, 46)).unwrap();
    app.help = true;
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    app.help = false;
    app.detail = true;
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    app.detail = false;
    app.records.clear();
    app.rebuild().unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    app.loading = true;
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
}
#[test]
fn export_visual_preview() {
    let Ok(path) = std::env::var("TOKEN_SPEED_SNAPSHOT") else {
        return;
    };
    let mut app = demo_app();
    app.range = app::Range::Today;
    for screen in args::Screen::ALL {
        app.set_screen(screen);
        if screen == args::Screen::Timeline {
            app.cursor = 9;
        }
        let mut term = Terminal::new(TestBackend::new(140, 46)).unwrap();
        term.draw(|f| ui::draw(f, &mut app)).unwrap();
        let hex = |c: Color| match c {
            Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            _ => unreachable!("palette colors are RGB"),
        };
        let (bg, ink) = ui::base_colors(app.args.theme);
        let (bg, ink) = (hex(bg), hex(ink));
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1540\" height=\"966\" viewBox=\"0 0 1540 966\"><rect width=\"100%\" height=\"100%\" fill=\"{bg}\"/><g font-family=\"Menlo,monospace\" font-size=\"16\">",
        );
        let color = |c: Color| -> String {
            match c {
                Color::Rgb(..) => hex(c),
                Color::Reset => bg.clone(),
                _ => ink.clone(),
            }
        };
        for y in 0..46 {
            for x in 0..140 {
                let c = &term.backend().buffer()[(x, y)];
                let symbol = c
                    .symbol()
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                svg.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"11\" height=\"21\" fill=\"{}\"/>",
                    x * 11,
                    y * 21,
                    color(c.bg)
                ));
                if symbol != " " {
                    svg.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" fill=\"{}\">{symbol}</text>",
                        x * 11,
                        y * 21 + 16,
                        color(c.fg)
                    ));
                }
            }
        }
        svg.push_str("</g></svg>");
        let file = if screen == args::Screen::Overview {
            path.clone()
        } else {
            path.strip_suffix(".svg").unwrap_or(&path).to_owned()
                + "-"
                + &screen.label().to_lowercase()
                + ".svg"
        };
        fs::write(file, svg).unwrap();
    }
}

#[test]
fn concurrent_cache_refreshes_do_not_deadlock() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("sessions")).unwrap();
    for i in 0..80 {
        fs::write(
            dir.path().join(format!("sessions/{i}.jsonl")),
            codex(
                10,
                "token_usage_record",
                json!({"response_id":format!("r{i}"), "usage":{"output_tokens":100}}),
            )
            .to_string(),
        )
        .unwrap();
    }
    let mut args = Args::parse_from(["token-speed", "--provider", "codex"]);
    args.codex_dir = dir.path().into();
    args.cache = dir.path().join("index.sqlite3");
    data::collect(&args, |_, _| {}).unwrap();
    args.refresh = true;
    let other = args.clone();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let signal = barrier.clone();
    let worker = std::thread::spawn(move || {
        signal.wait();
        data::collect(&other, |_, _| {})
    });
    barrier.wait();
    assert_eq!(data::collect(&args, |_, _| {}).unwrap().0.len(), 80);
    assert_eq!(worker.join().unwrap().unwrap().0.len(), 80);
}

#[test]
fn four_views_preserve_filters_and_distinguish_missing_timing() {
    let mut app = demo_app();
    let mut missing = app.records[0].clone();
    missing.timestamp = app.view.chart_start + 9.0 * 3600.0 + 10.0;
    missing.start = None;
    missing.id = "missing-timing".into();
    missing.provider = "codex".into();
    app.records.push(missing);
    app.records
        .sort_by(|a, b| a.timestamp.total_cmp(&b.timestamp));
    app.rebuild().unwrap();
    assert_eq!(app.view.requests[9][0], 1);
    assert_eq!(app.view.timed[9][0], 0);
    assert!(app.view.bins[9][0].is_none());
    app.search = "gpt".into();
    app.rebuild().unwrap();
    for screen in args::Screen::ALL {
        press(&mut app, KeyCode::F(screen.index() as u8 + 1));
        assert_eq!(app.screen, screen);
        assert_eq!(app.search, "gpt");
        assert!(app.view.rows.iter().all(|r| r.model.contains("gpt")));
    }
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.screen, args::Screen::Models);
    press(&mut app, KeyCode::F(2));
    app.cursor = 9;
    let mut terminal = Terminal::new(TestBackend::new(140, 46)).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("no usable timing"));
    press(&mut app, KeyCode::Char('g'));
    assert!(app.overlay);
    press(&mut app, KeyCode::Left);
    assert_eq!(app.cursor, 8);
    app.cursor = 0;
    press(&mut app, KeyCode::Left);
    assert_eq!(app.cursor, 0);
    press(&mut app, KeyCode::Enter);
    assert!(app.detail);
}

fn dsh(t: u64, typ: &str, data: Value) -> String {
    json!({"type":typ,"seq":t,"time":1_791_000_000_000u64 + t * 1000,"data":data}).to_string()
}
fn dsh_message(t: u64, step: u64, id: &str, output: u64) -> String {
    dsh(
        t,
        "assistant/message",
        json!({"turn":1,"step":step,"message":{"role":"assistant","id":id,
            "source":{"kind":"model","provider":"deepseek-official","model":"deepseek-test"}},
            "usage":{"inputTokens":10,"outputTokens":output,"cacheReadTokens":90,"reasoningTokens":40}}),
    )
}

#[test]
fn dsh_steps_cache_and_frames() {
    use ruzstd::encoding::{CompressionLevel, compress_to_vec};
    let lines = [
        json!({"type":"session","id":"session-synthetic","createdAt":1_791_000_000_000u64})
            .to_string(),
        dsh(1, "turn/start", json!({"turn":1})),
        dsh(2, "step/start", json!({"turn":1,"step":1})),
        dsh(
            4,
            "tool/call",
            json!({"turn":1,"step":1,"name":"bash","arguments":"synthetic"}),
        ),
        dsh_message(6, 1, "m1", 200),
        // A mirrored message keeps maximum counters instead of adding them.
        dsh_message(6, 1, "m1", 200),
        dsh(7, "step/start", json!({"turn":1,"step":2})),
        dsh_message(9, 2, "m2", 100),
        // No step/start: usage counts, timing stays missing.
        dsh_message(12, 3, "m3", 50),
    ];
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("sessions/--synthetic--/session-synthetic");
    fs::create_dir_all(&session).unwrap();
    // DSH appends one zstd frame per write; end with a frame cut mid-write.
    let mut bytes = Vec::new();
    for line in &lines {
        bytes.extend(compress_to_vec(
            format!("{line}\n").as_bytes(),
            CompressionLevel::Fastest,
        ));
    }
    let tail = compress_to_vec(
        dsh_message(20, 4, "m4", 999).as_bytes(),
        CompressionLevel::Fastest,
    );
    bytes.extend(&tail[..tail.len() / 2]);
    let path = session.join("session.jsonl.zstd");
    fs::write(&path, &bytes).unwrap();
    fs::write(session.join("session.lock"), "").unwrap();

    let (rows, bad) = data::parse_file(&path, "dsh").unwrap();
    assert_eq!(bad, 1);
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| r.session == "session-synthetic"));
    assert!(rows.iter().all(|r| r.model == "deepseek-test"));
    let (m1, m2, m3) = (&rows[0], &rows[1], &rows[2]);
    assert_eq!((m1.output_tokens, m1.reasoning_tokens), (200, 40));
    assert_eq!((m1.input_tokens, m1.cached_tokens), (100, 90));
    assert_eq!(m1.rate(300.0), Some(50.0));
    assert_eq!(m2.rate(300.0), Some(50.0));
    assert!(m3.start.is_none());
    let s = data::summarize(&rows, 300.0);
    assert_eq!((s.output_tokens, s.timed_output_tokens), (350, 300));
    assert_eq!(s.tokens_per_second, Some(50.0));

    let mut args = Args::parse_from(["token-speed", "--provider", "dsh", "--all-time"]);
    args.dsh_dir = dir.path().into();
    args.cache = dir.path().join("index.sqlite3");
    let (rows, meta) = data::collect(&args, |_, _| {}).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(meta.parsed_files, 1);
    assert!(rows.iter().all(|r| r.provider == "dsh"));
}

#[test]
fn dsh_source_appears_once_indexed() {
    let mut app = demo_app();
    assert_eq!(app.visible_sources(), vec![0, 1]);
    let mut extra = app.records[0].clone();
    extra.provider = "dsh".into();
    extra.id = "dsh-1".into();
    extra.model = "deepseek-test".into();
    app.records.push(extra);
    app.rebuild().unwrap();
    assert_eq!(app.visible_sources(), vec![0, 1, 2]);
    assert_eq!(app.view.sources[2].requests, 1);
    press(&mut app, KeyCode::Char('4'));
    assert!(app.view.rows.iter().all(|r| r.provider == "dsh"));
    for screen in args::Screen::ALL {
        app.set_screen(screen);
        for (width, height) in [(140, 46), (80, 24), (40, 12)] {
            let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
            term.draw(|f| ui::draw(f, &mut app)).unwrap();
        }
    }
    press(&mut app, KeyCode::Char('1'));
    app.set_screen(args::Screen::Timeline);
    let mut term = Terminal::new(TestBackend::new(140, 46)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    let text = term
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("4 DSH"));
    assert!(text.contains("DSH"));
}

fn draw_at(app: &mut app::App, width: u16, height: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|f| ui::draw(f, app)).unwrap();
    let buffer = term.backend().buffer();
    (0..height)
        .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
        .collect()
}
/// Click the first cell of `label` as rendered, the way a user would.
fn click_label(app: &mut app::App, width: u16, height: u16, label: &str) {
    let lines = draw_at(app, width, height);
    let (y, x) = lines
        .iter()
        .enumerate()
        .find_map(|(y, line)| {
            line.find(label)
                .map(|byte| (y, line[..byte].chars().count()))
        })
        .unwrap_or_else(|| panic!("{label:?} not rendered at {width}x{height}"));
    app.click(x as u16, y as u16).unwrap();
}

#[test]
fn theme_toggles_by_key_flag_and_click() {
    let mut app = demo_app();
    assert_eq!(app.args.theme, args::Theme::Dark);
    let background = |app: &mut app::App| {
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| ui::draw(f, app)).unwrap();
        term.backend().buffer()[(0, 0)].bg
    };
    let dark = background(&mut app);
    press(&mut app, KeyCode::Char('T'));
    assert_eq!(app.args.theme, args::Theme::Light);
    let light = background(&mut app);
    assert_ne!(dark, light);
    assert_eq!(
        (dark, light),
        (
            ui::base_colors(args::Theme::Dark).0,
            ui::base_colors(args::Theme::Light).0
        )
    );
    assert!(
        draw_at(&mut app, 120, 40)
            .iter()
            .any(|l| l.contains("Light · v"))
    );
    click_label(&mut app, 120, 40, "◐ Light");
    assert!(
        draw_at(&mut app, 120, 40)
            .iter()
            .any(|l| l.contains("Dark · v"))
    );
    assert_eq!(app.args.theme, args::Theme::Dark);
    let args = Args::parse_from(["token-speed", "--theme", "light"]);
    assert_eq!(args.theme, args::Theme::Light);
}

#[test]
fn clicks_hit_rendered_controls_at_every_width() {
    // Chips sit on top of right-aligned and narrow layouts; each click must
    // land on the control actually drawn under the pointer.
    for (width, height) in [(140, 46), (120, 40), (100, 30), (80, 24), (60, 20)] {
        let mut app = demo_app();
        click_label(&mut app, width, height, "3 Claude");
        assert_eq!(app.args.provider, args::Provider::Claude, "{width}");
        assert!(app.view.rows.iter().all(|r| r.provider == "claude"));
        click_label(&mut app, width, height, "1 All");
        assert_eq!(app.args.provider, args::Provider::All, "{width}");
        click_label(&mut app, width, height, "F3 Models");
        assert_eq!(app.screen, args::Screen::Models, "{width}");
        // Short tab labels below 80 columns inside the margin.
        click_label(&mut app, width, height, "F2 Time");
        assert_eq!(app.screen, args::Screen::Timeline, "{width}");
        // Below 102 columns (100 inside the margin) the toolbar is compact.
        if width >= 102 {
            click_label(&mut app, width, height, "30 days");
            assert_eq!(app.range, app::Range::Month, "{width}");
            click_label(&mut app, width, height, "Today");
            assert_eq!(app.range, app::Range::Today, "{width}");
        } else {
            let range = app.range;
            click_label(&mut app, width, height, range.label());
            assert_ne!(app.range, range, "{width}");
        }
        // Gaps between chips do nothing.
        let before = (app.args.provider, app.screen, app.range);
        let lines = draw_at(&mut app, width, height);
        let y = lines.iter().position(|l| l.contains("1 All")).unwrap();
        // The chip " 1 All " starts one cell before the label; the next cell
        // after it is the single-space gap.
        let label = lines[y].find("1 All").unwrap();
        let x = lines[y][..label].chars().count() - 1 + " 1 All ".len();
        app.click(x as u16, y as u16).unwrap();
        assert_eq!(before, (app.args.provider, app.screen, app.range));
    }
}

#[test]
fn clicks_select_rows_buckets_and_close_dialogs() {
    let mut app = demo_app();
    app.set_screen(args::Screen::Requests);
    let lines = draw_at(&mut app, 140, 46);
    let header = lines.iter().position(|l| l.contains("Completed")).unwrap();
    // The first body row is two lines under the header (header margin).
    app.click(20, header as u16 + 4).unwrap();
    assert_eq!(app.table.selected(), Some(2));
    assert!(!app.detail);
    app.click(20, header as u16 + 4).unwrap();
    assert!(app.detail, "second click on the selected row opens details");
    app.click(0, 0).unwrap();
    assert!(!app.detail, "a click dismisses the dialog");
    // Rows below the end of the table select nothing.
    app.records.truncate(2);
    app.rebuild().unwrap();
    draw_at(&mut app, 140, 46);
    let selected = app.table.selected();
    app.click(20, header as u16 + 10).unwrap();
    assert_eq!(app.table.selected(), selected);
    assert!(!app.detail);

    // Scrolled tables map clicks through the scroll offset.
    let mut app = demo_app();
    app.set_screen(args::Screen::Requests);
    for _ in 0..120 {
        press(&mut app, KeyCode::Down);
    }
    draw_at(&mut app, 140, 46);
    let offset = app.table.offset();
    assert!(offset > 0);
    app.click(20, header as u16 + 2).unwrap();
    assert_eq!(app.table.selected(), Some(offset));

    // The coverage strip and the plot move the bucket cursor.
    let mut app = demo_app();
    app.set_screen(args::Screen::Timeline);
    let lines = draw_at(&mut app, 140, 46);
    let y = lines
        .iter()
        .position(|l| l.contains("Codex  ") && l.contains('█'))
        .unwrap();
    let start = lines[y].find("Codex  ").unwrap();
    let start = lines[y][..start].chars().count() + 7;
    app.click(start as u16, y as u16).unwrap();
    assert_eq!(app.cursor, 0);
    let last = lines[y]
        .chars()
        .enumerate()
        .filter(|(_, c)| "░!▁▂▃▄▅▆▇█".contains(*c))
        .map(|(x, _)| x)
        .max()
        .unwrap();
    app.click(last as u16, y as u16).unwrap();
    assert_eq!(app.cursor, app.view.bins.len() - 1);

    // A click while typing a filter keeps it and still acts.
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('g'));
    click_label(&mut app, 140, 46, "F4 Requests");
    assert!(!app.editing);
    assert_eq!(app.search, "g");
    assert_eq!(app.screen, args::Screen::Requests);
}

#[test]
fn idle_mouse_events_are_ignored_but_clicks_redraw() {
    use crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
    let mut app = demo_app();
    let mouse = |kind| {
        Event::Mouse(MouseEvent {
            kind,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        })
    };
    for kind in [
        MouseEventKind::Moved,
        MouseEventKind::Up(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Down(MouseButton::Right),
    ] {
        assert!(matches!(
            app.event(mouse(kind)).unwrap(),
            app::Handled::Ignored
        ));
    }
    assert!(matches!(
        app.event(mouse(MouseEventKind::Down(MouseButton::Left)))
            .unwrap(),
        app::Handled::Changed
    ));
}
