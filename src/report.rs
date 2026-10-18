use crate::{
    args::{Args, Command, Format},
    data::{self, Clock, Record},
    source::Source,
    ui,
};
use anyhow::Result;
use ratatui::text::Line;
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    time::Instant,
};

fn enriched(r: &Record, clock: Clock, max_gap: f64) -> Value {
    let mut value = serde_json::to_value(r).unwrap();
    let status = if r.duration(max_gap).is_some() {
        "estimated"
    } else if r.start.is_none() {
        "missing"
    } else {
        "excluded"
    };
    let o = value.as_object_mut().unwrap();
    o.insert("seconds".into(), json!(r.duration(max_gap)));
    o.insert("tokens_per_second".into(), json!(r.rate(max_gap)));
    o.insert("timing_status".into(), json!(status));
    o.insert(
        "time".into(),
        json!(clock.format(r.timestamp, "%Y-%m-%dT%H:%M:%S%:z")),
    );
    value
}
pub fn run(args: &Args, clock: Clock) -> Result<()> {
    let mut last = Instant::now();
    let (rows, meta) = data::collect(args, |p, c| {
        if last.elapsed().as_secs() >= 10 {
            eprintln!("Indexing local logs: {p} parsed, {c} cached…");
            last = Instant::now();
        }
    })?;
    let rows = data::select(&rows, args, clock)?;
    let mut out = io::BufWriter::new(io::stdout().lock());
    write_report(&mut out, args, clock, &rows, &meta)?;
    out.flush()?;
    Ok(())
}

pub(crate) fn write_report(
    mut out: impl Write,
    args: &Args,
    clock: Clock,
    rows: &[Record],
    meta: &data::Metadata,
) -> Result<()> {
    let recent = args.command == Command::Recent;
    let selected = if recent {
        &rows[rows.len().saturating_sub(args.limit)..]
    } else {
        rows
    };
    let summary = data::aggregate(rows, args.group, clock, args.max_gap);
    let output = if recent {
        selected
            .iter()
            .map(|r| enriched(r, clock, args.max_gap))
            .collect::<Vec<_>>()
    } else {
        summary
            .iter()
            .map(|s| serde_json::to_value(s).unwrap())
            .collect()
    };
    match args.output_format() {
        Format::Json => {
            serde_json::to_writer_pretty(
                &mut out,
                &json!({"metric":"estimated_request_output_tokens_per_second","max_gap_seconds":args.max_gap,"timezone":clock.label(),"metadata":meta,"data":output}),
            )?;
            writeln!(out)?;
        }
        Format::Csv => {
            if let Some(first) = output.first().and_then(Value::as_object) {
                let columns: Vec<_> = first.keys().cloned().collect();
                let mut writer = csv::Writer::from_writer(&mut out);
                writer.write_record(&columns)?;
                for row in &output {
                    writer.write_record(columns.iter().map(|c| {
                        let v = &row[c];
                        if let Some(s) = v.as_str() {
                            s.into()
                        } else if v.is_null() {
                            String::new()
                        } else {
                            v.to_string()
                        }
                    }))?;
                }
                writer.flush()?;
            }
        }
        Format::Table => {
            writeln!(
                out,
                "Token Speed · estimated request throughput · output tokens/s"
            )?;
            writeln!(
                out,
                "Timing includes prefill, reasoning, network and request gaps; intervals > {}s excluded.",
                args.max_gap
            )?;
            let (headers, table) = if recent {
                (
                    vec![
                        "Time",
                        "Provider",
                        "Model",
                        "Mode",
                        "Output",
                        "Seconds",
                        "Est. tok/s",
                        "Session",
                    ],
                    selected
                        .iter()
                        .map(|r| {
                            vec![
                                clock.format(r.timestamp, "%Y-%m-%d %H:%M:%S"),
                                r.provider.id().into(),
                                r.model.clone(),
                                r.speed_mode.clone().unwrap_or_else(|| "—".into()),
                                ui::grouped(r.output_tokens),
                                ui::rate(r.duration(args.max_gap)),
                                ui::rate(r.rate(args.max_gap)),
                                r.session.chars().take(12).collect(),
                            ]
                        })
                        .collect::<Vec<_>>(),
                )
            } else {
                (
                    vec![
                        "Period",
                        "Provider",
                        "Model",
                        "Req",
                        "Timed",
                        "Input",
                        "Cached",
                        "Output",
                        "Est. tok/s",
                        "Median",
                    ],
                    summary.iter().map(summary_cells).collect(),
                )
            };
            if table.is_empty() {
                writeln!(out, "No matching token records.")?;
            } else {
                let total = if recent {
                    None
                } else {
                    let mut cells = summary_cells(&data::summarize(rows, args.max_gap));
                    cells[0] = "Total".into();
                    Some(cells)
                };
                let numeric = if recent {
                    &[4, 5, 6][..]
                } else {
                    &[3, 4, 5, 6, 7, 8, 9][..]
                };
                write_table(&mut out, &headers, table, numeric, total)?;
            }
            writeln!(
                out,
                "\nFiles: {} parsed, {} cached.",
                meta.parsed_files, meta.cached_files
            )?;
            if !meta.warnings.is_empty() {
                eprintln!(
                    "{} warnings; use --format json for details",
                    meta.warnings.len()
                );
            }
        }
    }
    out.flush()?;
    Ok(())
}

fn summary_cells(s: &data::Summary) -> Vec<String> {
    vec![
        s.period.clone(),
        s.provider.map_or("", Source::id).into(),
        s.model.clone(),
        ui::grouped(s.requests),
        format!(
            "{}/{}",
            ui::grouped(s.timed_requests),
            ui::grouped(s.requests)
        ),
        ui::grouped(s.input_tokens),
        ui::grouped(s.cached_tokens),
        ui::grouped(s.output_tokens),
        ui::rate(s.tokens_per_second),
        ui::rate(s.median_tokens_per_second),
    ]
}

fn write_table(
    mut out: impl Write,
    headers: &[&str],
    rows: Vec<Vec<String>>,
    numeric: &[usize],
    total: Option<Vec<String>>,
) -> io::Result<()> {
    // Log metadata must stay within one cell and cannot inject terminal controls.
    let clean = |row: Vec<String>| {
        row.into_iter()
            .map(|s| {
                s.chars()
                    .map(|c| if c.is_control() { ' ' } else { c })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
    };
    let rows: Vec<_> = rows.into_iter().map(clean).collect();
    let total = total.map(clean);
    let widths: Vec<_> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            rows.iter()
                .chain(total.iter())
                .map(|r| Line::from(r[i].as_str()).width())
                .max()
                .unwrap_or(0)
                .max(Line::from(*h).width())
        })
        .collect();
    let border = |out: &mut dyn Write, left: char, joint: char, right: char| {
        write!(out, "{left}")?;
        for (i, width) in widths.iter().enumerate() {
            if i > 0 {
                write!(out, "{joint}")?;
            }
            write!(out, "{}", "─".repeat(width + 2))?;
        }
        writeln!(out, "{right}")
    };
    let row = |out: &mut dyn Write, cells: &[String], align_numbers: bool| {
        write!(out, "│")?;
        for (i, cell) in cells.iter().enumerate() {
            let padding = " ".repeat(widths[i].saturating_sub(Line::from(cell.as_str()).width()));
            if align_numbers && numeric.contains(&i) {
                write!(out, " {padding}{cell} │")?;
            } else {
                write!(out, " {cell}{padding} │")?;
            }
        }
        writeln!(out)
    };
    border(&mut out, '┌', '┬', '┐')?;
    row(
        &mut out,
        &headers.iter().map(|h| h.to_string()).collect::<Vec<_>>(),
        false,
    )?;
    border(&mut out, '├', '┼', '┤')?;
    for cells in &rows {
        row(&mut out, cells, true)?;
    }
    if let Some(total) = total {
        border(&mut out, '├', '┼', '┤')?;
        row(&mut out, &total, true)?;
    }
    border(&mut out, '└', '┴', '┘')
}
