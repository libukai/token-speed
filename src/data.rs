//! Indexing (with the SQLite cache), selection and throughput statistics.
//! Parsing lives in `parse`; the list of sources in `source`.

use crate::{
    args::{Args, Group},
    source::{Filter, Source},
};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::UNIX_EPOCH,
};
use walkdir::WalkDir;

const CACHE_VERSION: i64 = 8;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub provider: Source,
    pub session: String,
    pub id: String,
    pub model: String,
    pub timestamp: f64,
    pub start: Option<f64>,
    pub input_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub timing_source: String,
    /// Codex `service_tier` or Claude `usage.speed`; `None` when not logged.
    #[serde(default)]
    pub speed_mode: Option<String>,
}
impl Record {
    pub fn duration(&self, max_gap: f64) -> Option<f64> {
        let d = self.timestamp - self.start?;
        (d > 0.0 && d <= max_gap && self.output_tokens > 0).then_some(d)
    }
    pub fn rate(&self, max_gap: f64) -> Option<f64> {
        Some(self.output_tokens as f64 / self.duration(max_gap)?)
    }
    /// Identity of one call. Claude message ids are global, and a resumed
    /// session copies earlier messages into its new file, so Claude calls
    /// are keyed by message alone.
    pub fn key(&self) -> (Source, String, String) {
        let session = match self.provider {
            Source::Claude => String::new(),
            _ => self.session.clone(),
        };
        (self.provider, session, self.id.clone())
    }
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Metadata {
    pub parsed_files: usize,
    pub cached_files: usize,
    pub warnings: Vec<String>,
}

pub fn collect(
    args: &Args,
    mut progress: impl FnMut(usize, usize),
) -> Result<(Vec<Record>, Metadata)> {
    use crate::parse::magpie::GATEWAY;
    if let Some(parent) = args.cache.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut db = Connection::open(&args.cache)?;
    db.busy_timeout(std::time::Duration::from_secs(5))?;
    // Readers never wait for a writer in WAL mode. Lookups run outside any
    // transaction and freshly parsed files are stored in short batches, so a
    // cold index never holds the write lock for long.
    db.pragma_update(None, "journal_mode", "WAL")?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, provider TEXT, size INTEGER, mtime INTEGER, version INTEGER, rows TEXT, malformed INTEGER);")?;
    let roots = Source::ALL
        .into_iter()
        // The gateway ledger files calls under the agents that made them, so
        // it is read whichever source is selected, hidden or not.
        .filter(|s| *s == Source::Other || args.provider.matches(*s))
        .flat_map(|s| {
            args.dirs(s)
                .iter()
                .flat_map(move |d| s.spec().roots.iter().map(move |r| (s, d.join(r))))
        });
    let mut meta = Metadata::default();
    let mut result = HashMap::new();
    let mut paths = HashSet::new();
    let mut pending = Vec::new();
    for (source, root) in roots {
        let provider = source.id();
        if !root.exists() {
            // Optional sources stay quiet unless explicitly requested.
            if !source.spec().optional || args.provider == Filter::only(source) {
                meta.warnings
                    .push(format!("Missing log directory: {}", root.display()));
            }
            continue;
        }
        for entry in WalkDir::new(&root).into_iter() {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    meta.warnings.push(e.to_string());
                    continue;
                }
            };
            let path = entry.path();
            let name = entry.file_name().to_string_lossy();
            if !entry.file_type().is_file() || !(source.spec().is_log)(&name) {
                continue;
            }
            // A file can vanish or move mid-walk (Codex archives sessions);
            // skip it with a warning rather than fail the whole pass.
            let (absolute, stat) = match path.canonicalize().and_then(|a| Ok((a, path.metadata()?)))
            {
                Ok(v) => v,
                Err(e) => {
                    meta.warnings.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            if !paths.insert(absolute.clone()) {
                continue;
            }
            let key = absolute.to_string_lossy().into_owned();
            let mtime = stat
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos() as i64);
            let saved:Option<(i64,i64,i64,String,usize)>=db.query_row("SELECT size,mtime,version,rows,malformed FROM files WHERE path=? AND provider=?",params![key,provider],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get::<_,i64>(4)? as usize))).optional()?;
            let cached = saved.filter(|s| {
                !args.refresh && (s.0, s.1, s.2) == (stat.len() as i64, mtime, CACHE_VERSION)
            });
            let decoded = cached.and_then(|s| {
                serde_json::from_str::<Vec<Record>>(&s.3)
                    .ok()
                    .map(|rows| (rows, s.4))
            });
            let (rows, bad) = if let Some(value) = decoded {
                meta.cached_files += 1;
                value
            } else {
                let (rows, bad) = match source.parse(path) {
                    Ok(v) => v,
                    Err(e) => {
                        meta.warnings.push(format!("{}: {e}", path.display()));
                        continue;
                    }
                };
                pending.push((
                    key,
                    provider,
                    stat.len() as i64,
                    mtime,
                    serde_json::to_string(&rows)?,
                    bad as i64,
                ));
                if pending.len() >= 16 {
                    store(&mut db, &mut pending)?;
                }
                meta.parsed_files += 1;
                (rows, bad)
            };
            if bad > 0 {
                meta.warnings.push(format!(
                    "{}: {bad} malformed/incomplete lines skipped",
                    path.display()
                ));
            }
            for r in rows {
                let k = r.key();
                // The later record wins, but a gateway copy never replaces
                // the agent's own.
                let rank = |r: &Record| (r.timing_source != GATEWAY, r.timestamp);
                if result
                    .get(&k)
                    .is_none_or(|prev: &Record| rank(&r) > rank(prev))
                {
                    result.insert(k, r);
                }
            }
            if (meta.parsed_files + meta.cached_files) % 64 == 0 {
                progress(meta.parsed_files, meta.cached_files);
            }
        }
    }
    store(&mut db, &mut pending)?;
    progress(meta.parsed_files, meta.cached_files);
    let mut rows = drop_mirrored(result.into_values().collect(), args.max_gap);
    rows.sort_by(|a, b| a.timestamp.total_cmp(&b.timestamp));
    Ok((rows, meta))
}

type Pending = (String, &'static str, i64, i64, String, i64);
fn store(db: &mut Connection, pending: &mut Vec<Pending>) -> Result<()> {
    if pending.is_empty() {
        return Ok(());
    }
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    for (path, provider, size, mtime, rows, bad) in pending.drain(..) {
        tx.execute(
            "INSERT OR REPLACE INTO files VALUES (?,?,?,?,?,?,?)",
            params![path, provider, size, mtime, CACHE_VERSION, rows, bad],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Calls the gateway ledger shares with an agent's own log are counted once,
/// from the agent's log. The two have no call id in common, so a ledger
/// record is dropped when its completion falls within `SLACK` of a native
/// request's span (its start, when usable under `max_gap`, to its end) of
/// the same agent: of the same session when the ledger names the
/// agent's own session (Codex, Claude), else of any session. The
/// rest (an agent with no log, or the stretches where the log is missing)
/// stay.
const SLACK: f64 = 10.0;
fn drop_mirrored(rows: Vec<Record>, max_gap: f64) -> Vec<Record> {
    use crate::parse::magpie::{GATEWAY, NO_SESSION};
    type Spans = Vec<(f64, f64)>;
    let mut by_source: HashMap<Source, Spans> = HashMap::new();
    let mut by_session: HashMap<(Source, &str), Spans> = HashMap::new();
    for r in rows.iter().filter(|r| r.timing_source != GATEWAY) {
        // Only a plausible start widens the span; a stale or inverted one
        // would hide unrelated gateway calls.
        let start = r
            .start
            .filter(|s| r.timestamp - s > 0.0 && r.timestamp - s <= max_gap)
            .unwrap_or(r.timestamp);
        let span = (start - SLACK, r.timestamp + SLACK);
        by_source.entry(r.provider).or_default().push(span);
        by_session
            .entry((r.provider, r.session.as_str()))
            .or_default()
            .push(span);
    }
    let merge = |spans: &mut Spans| {
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Spans = Vec::new();
        for &(from, to) in spans.iter() {
            match merged.last_mut() {
                Some(last) if from <= last.1 => last.1 = last.1.max(to),
                _ => merged.push((from, to)),
            }
        }
        *spans = merged;
    };
    by_source.values_mut().for_each(merge);
    by_session.values_mut().for_each(merge);
    let covers = |spans: Option<&Spans>, at: f64| {
        spans.is_some_and(|spans| {
            let i = spans.partition_point(|s| s.1 < at);
            spans.get(i).is_some_and(|s| s.0 <= at)
        })
    };
    let mirrored: Vec<bool> = rows
        .iter()
        .map(|r| {
            r.timing_source == GATEWAY
                && if r.session == NO_SESSION || r.provider == Source::Other {
                    covers(by_source.get(&r.provider), r.timestamp)
                } else {
                    covers(
                        by_session.get(&(r.provider, r.session.as_str())),
                        r.timestamp,
                    )
                }
        })
        .collect();
    rows.into_iter()
        .zip(mirrored)
        .filter_map(|(r, mirrored)| (!mirrored).then_some(r))
        .collect()
}

#[derive(Clone, Copy, Debug)]
pub struct Clock(pub Option<Tz>);
impl Clock {
    pub fn new(name: Option<&str>) -> Result<Self> {
        Ok(Self(
            name.map(|s| s.parse::<Tz>())
                .transpose()
                .context("Invalid IANA timezone")?,
        ))
    }
    pub fn label(self) -> String {
        self.0
            .map(|z| z.name().to_owned())
            .unwrap_or_else(|| Local::now().format("%Z").to_string())
    }
    pub fn format(self, stamp: f64, fmt: &str) -> String {
        let d = DateTime::<Utc>::from_timestamp_millis((stamp * 1000.0) as i64).unwrap_or_default();
        match self.0 {
            Some(tz) => d.with_timezone(&tz).format(fmt).to_string(),
            None => d.with_timezone(&Local).format(fmt).to_string(),
        }
    }
    pub fn boundary(self, s: &str, end: bool) -> Result<f64> {
        if let Ok(d) = DateTime::parse_from_rfc3339(s) {
            return Ok(d.timestamp_millis() as f64 / 1000.0);
        }
        let d = if s.len() == 10 {
            let date = NaiveDate::parse_from_str(s, "%Y-%m-%d").context("Invalid date")?;
            let date = if end { date + Duration::days(1) } else { date };
            date.and_hms_opt(0, 0, 0).unwrap()
        } else {
            NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
                .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M"))
                .context("Invalid ISO time")?
        };
        // A calendar day starts at its first instant: where DST skips
        // midnight, that is the end of the gap. Clock times inside a gap or
        // overlap are ambiguous and need an explicit offset.
        let first = |d: NaiveDateTime| match self.0 {
            Some(tz) => tz.from_local_datetime(&d).earliest().map(|d| d.timestamp()),
            None => Local
                .from_local_datetime(&d)
                .earliest()
                .map(|d| d.timestamp()),
        };
        let exact = |d: NaiveDateTime| match self.0 {
            Some(tz) => tz.from_local_datetime(&d).single().map(|d| d.timestamp()),
            None => Local
                .from_local_datetime(&d)
                .single()
                .map(|d| d.timestamp()),
        };
        let stamp = if s.len() == 10 {
            // DST gaps are at most a few hours; step to the first valid minute.
            (0..=24 * 60).find_map(|m| first(d + Duration::minutes(m)))
        } else {
            exact(d)
        };
        stamp
            .map(|t| t as f64)
            .context("Ambiguous or nonexistent local time; supply an explicit UTC offset")
    }
}
pub fn bounds(args: &Args, clock: Clock) -> Result<(Option<f64>, Option<f64>)> {
    let now = Utc::now().timestamp_millis() as f64 / 1000.0;
    let since = if let Some(s) = &args.since {
        Some(clock.boundary(s, false)?)
    } else if args.today {
        Some(clock.boundary(&clock.format(now, "%Y-%m-%d"), false)?)
    } else if args.all_time {
        None
    } else {
        Some(now - args.days as f64 * 86400.0)
    };
    let until = args
        .until
        .as_deref()
        .map(|s| clock.boundary(s, true))
        .transpose()?;
    if matches!((since,until),(Some(s),Some(e)) if e<=s) {
        bail!("--until must be after --since");
    }
    Ok((since, until))
}
pub fn select(rows: &[Record], args: &Args, clock: Clock) -> Result<Vec<Record>> {
    let (since, until) = bounds(args, clock)?;
    Ok(rows
        .iter()
        .filter(|r| {
            args.provider.matches(r.provider)
                // A hidden source is left out unless it is asked for.
                && (!r.provider.spec().hidden || args.provider == Filter::only(r.provider))
                && since.is_none_or(|s| r.timestamp >= s)
                && until.is_none_or(|u| r.timestamp < u)
                && args
                    .model
                    .as_ref()
                    .is_none_or(|m| r.model.to_lowercase().contains(&m.to_lowercase()))
                && args.session.as_ref().is_none_or(|s| r.session.contains(s))
        })
        .cloned()
        .collect())
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Summary {
    pub period: String,
    pub provider: Option<Source>,
    pub model: String,
    pub requests: usize,
    pub timed_requests: usize,
    pub input_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub timed_output_tokens: u64,
    pub seconds: f64,
    pub tokens_per_second: Option<f64>,
    pub median_tokens_per_second: Option<f64>,
}
pub fn summarize(
    rows: impl IntoIterator<Item = impl std::borrow::Borrow<Record>>,
    max_gap: f64,
) -> Summary {
    let mut s = Summary::default();
    let mut rates = Vec::new();
    for r in rows {
        let r = r.borrow();
        s.requests += 1;
        s.input_tokens = s.input_tokens.saturating_add(r.input_tokens);
        s.cached_tokens = s.cached_tokens.saturating_add(r.cached_tokens);
        s.cache_write_tokens = s.cache_write_tokens.saturating_add(r.cache_write_tokens);
        s.output_tokens = s.output_tokens.saturating_add(r.output_tokens);
        s.reasoning_tokens = s.reasoning_tokens.saturating_add(r.reasoning_tokens);
        if let Some(d) = r.duration(max_gap) {
            s.timed_requests += 1;
            s.seconds += d;
            s.timed_output_tokens = s.timed_output_tokens.saturating_add(r.output_tokens);
            rates.push(r.output_tokens as f64 / d);
        }
    }
    if s.seconds > 0.0 {
        s.tokens_per_second = Some(s.timed_output_tokens as f64 / s.seconds);
    }
    rates.sort_by(f64::total_cmp);
    let len = rates.len();
    if len > 0 {
        s.median_tokens_per_second = Some(if len % 2 == 0 {
            (rates[len / 2 - 1] + rates[len / 2]) / 2.0
        } else {
            rates[len / 2]
        });
    }
    s
}
pub fn aggregate(rows: &[Record], group: Group, clock: Clock, max_gap: f64) -> Vec<Summary> {
    let mut bins: BTreeMap<(String, Source, String), Vec<&Record>> = BTreeMap::new();
    for r in rows {
        let period = match group {
            Group::Hour => clock.format(r.timestamp, "%Y-%m-%d %H:00 %z"),
            Group::Day => clock.format(r.timestamp, "%Y-%m-%d"),
            Group::Model => "all".into(),
        };
        bins.entry((period, r.provider, r.model.clone()))
            .or_default()
            .push(r);
    }
    bins.into_iter()
        .map(|((period, provider, model), rs)| Summary {
            period,
            provider: Some(provider),
            model,
            ..summarize(rs, max_gap)
        })
        .collect()
}
