//! Registry of supported log sources.
//!
//! To add a source:
//! 1. add a `Source` variant, list it in `ALL` and the check below, bump `COUNT`, and
//!    add its `Spec` to `SPECS` in the same order;
//! 2. write `parse/<id>.rs` returning `Record`s (see `docs/metrics.md`);
//! 3. add a `--<id>-dir` flag in `args.rs`.
//!
//! The compiler then flags each remaining place (`Args::dir`, the palette
//! accent); keys, chips, cards, charts, reports and the cache follow from
//! the registry.

use crate::parse;
use anyhow::Result;
use clap::{ValueEnum, builder::PossibleValue};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A log source. Serialized as its lowercase id in the cache and exports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Codex,
    Claude,
    Dsh,
    Other,
}

pub struct Spec {
    /// Stable id: CLI value, cache key and export value.
    pub id: &'static str,
    /// Display name.
    pub label: &'static str,
    /// Environment variable overriding the home directory.
    pub env: &'static str,
    /// Home directory relative to `$HOME`.
    pub home: &'static str,
    /// Log roots under the home directory.
    pub roots: &'static [&'static str],
    /// Hidden until its logs are indexed, and silent when its home is
    /// missing, unless explicitly selected.
    pub optional: bool,
    /// Left out of every view and report, and with no tab or digit key,
    /// unless `--provider` names it.
    pub hidden: bool,
    /// Whether a file name is a log of this source.
    pub is_log: fn(&str) -> bool,
    pub parse: fn(&Path) -> Result<parse::Parsed>,
}

const SPECS: [Spec; Source::COUNT] = [
    Spec {
        id: "codex",
        label: "Codex",
        env: "CODEX_HOME",
        home: ".codex",
        roots: &["sessions", "archived_sessions"],
        optional: false,
        hidden: false,
        is_log: parse::is_jsonl,
        parse: parse::codex::parse,
    },
    Spec {
        id: "claude",
        label: "Claude",
        env: "CLAUDE_CONFIG_DIR",
        home: ".claude",
        roots: &["projects"],
        optional: false,
        hidden: false,
        is_log: parse::is_jsonl,
        parse: parse::claude::parse,
    },
    Spec {
        id: "dsh",
        label: "DSH",
        env: "DSH_HOME",
        home: ".dsh",
        roots: &["sessions"],
        optional: true,
        hidden: false,
        is_log: parse::dsh::is_log,
        parse: parse::dsh::parse,
    },
    Spec {
        id: "other",
        label: "Other",
        env: "MAGPIE_HOME",
        home: ".config/magpie",
        roots: &["usage.jsonl"],
        optional: true,
        hidden: true,
        is_log: parse::is_jsonl,
        parse: parse::magpie::parse,
    },
];

// Keeps `ALL` and `COUNT` in step with the enum: a new variant fails this
// match until it is listed, and `ALL[i]` must be variant `i`. `SPECS` has
// `COUNT` entries by type; tests check each lines up with its id.
const _: () = {
    let mut i = 0;
    while i < Source::COUNT {
        assert!(Source::ALL[i] as usize == i);
        i += 1;
    }
    match Source::Codex {
        Source::Codex | Source::Claude | Source::Dsh | Source::Other => {}
    }
};

impl Source {
    pub const COUNT: usize = 4;
    /// Display order; `index` positions per-source arrays.
    pub const ALL: [Self; Self::COUNT] = [Self::Codex, Self::Claude, Self::Dsh, Self::Other];

    pub fn index(self) -> usize {
        self as usize
    }
    pub fn spec(self) -> &'static Spec {
        &SPECS[self.index()]
    }
    pub fn id(self) -> &'static str {
        self.spec().id
    }
    pub fn label(self) -> &'static str {
        self.spec().label
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|s| s.id().eq_ignore_ascii_case(id))
    }
    /// The home from its environment variable, or the default.
    pub fn default_dir(self) -> PathBuf {
        let spec = self.spec();
        std::env::var_os(spec.env)
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::args::home().join(spec.home))
    }
    pub fn parse(self, path: &Path) -> Result<parse::Parsed> {
        (self.spec().parse)(path)
    }
}

/// Reports order sources by id, as they did when sources were strings.
impl Ord for Source {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id().cmp(other.id())
    }
}
impl PartialOrd for Source {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `pad` honors width and alignment, e.g. `{:<6}` in column layouts.
        f.pad(self.id())
    }
}

impl<T> std::ops::Index<Source> for [T; Source::COUNT] {
    type Output = T;
    fn index(&self, s: Source) -> &T {
        &self[s.index()]
    }
}
impl<T> std::ops::IndexMut<Source> for [T; Source::COUNT] {
    fn index_mut(&mut self, s: Source) -> &mut T {
        &mut self[s.index()]
    }
}

/// `--provider`: every source, or exactly one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filter(pub Option<Source>);

const FILTERS: [Filter; Source::COUNT + 1] = {
    let mut all = [Filter(None); Source::COUNT + 1];
    let mut i = 0;
    while i < Source::COUNT {
        all[i + 1] = Filter(Some(Source::ALL[i]));
        i += 1;
    }
    all
};

impl Filter {
    pub const ALL: Self = Self(None);
    pub fn only(s: Source) -> Self {
        Self(Some(s))
    }
    pub fn matches(self, s: Source) -> bool {
        self.0.is_none_or(|only| only == s)
    }
    /// Filter selected by a digit key: 1 is all, 2.. follow `Source::ALL`
    /// without the hidden sources.
    pub fn from_digit(d: u32) -> Option<Self> {
        let key = d.checked_sub(1)? as usize;
        FILTERS.iter().copied().filter(|f| f.keyed()).nth(key)
    }
    /// Whether a digit key selects this filter: all, or a source that is
    /// not hidden.
    pub fn keyed(self) -> bool {
        self.0.is_none_or(|s| !s.spec().hidden)
    }
    /// The digit key that selects this filter.
    pub fn digit(self) -> u32 {
        let at = FILTERS.iter().position(|f| *f == self).unwrap_or(0);
        FILTERS[..at].iter().filter(|f| f.keyed()).count() as u32 + 1
    }
    pub fn label(self) -> &'static str {
        self.0.map_or("All", Source::label)
    }
}

impl ValueEnum for Filter {
    fn value_variants<'a>() -> &'a [Self] {
        &FILTERS
    }
    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.0.map_or("all", Source::id)))
    }
}
