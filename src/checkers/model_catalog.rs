//! Catalog of AI model names and their lifecycle status.
//!
//! Prose and API-ID spellings of the same model (`Claude 3.5 Sonnet`,
//! `claude-3-5-sonnet-20241022`, `Claude Sonnet 3.5`) are normalised to one
//! canonical key (`claude-3-5-sonnet`) and looked up in a static table. Any
//! trailing snapshot date is extracted so unknown models can still be judged
//! by age.
//!
//! Catalog as of 2026-09. Refresh it at release time (see CONTRIBUTING.md);
//! users can patch it without a release via `current_models` in the config.

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// A snapshot date embedded in a model ID, as `(year, month, day)`.
pub(crate) type Snapshot = (i64, u32, u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelStatus {
    /// Recommended for new work. Never flagged; also exempt from the snapshot-age check.
    Current,
    /// Still served, but a newer generation exists.
    Superseded,
    /// Retirement announced; still served for now.
    Deprecated,
    /// No longer served; requests to it fail.
    Retired,
}

pub(crate) struct ModelEntry {
    pub name: &'static str,
    pub status: ModelStatus,
    pub successor: Option<&'static str>,
    keys: &'static [&'static str],
}

use ModelStatus::{Current, Deprecated, Retired, Superseded};

macro_rules! entry {
    ($name:literal, $status:expr, $succ:expr, [$($key:literal),+ $(,)?]) => {
        ModelEntry { name: $name, status: $status, successor: $succ, keys: &[$($key),+] }
    };
}

static BUILTIN: &[ModelEntry] = &[
    // ── Anthropic ───────────────────────────────────────────────────────
    entry!(
        "Claude 1",
        Retired,
        Some("claude-sonnet-5"),
        ["claude-v1", "claude-1", "claude-1-2", "claude-1-3"]
    ),
    entry!(
        "Claude Instant",
        Retired,
        Some("claude-haiku-4-5"),
        ["claude-instant", "claude-instant-1", "claude-instant-1-2"]
    ),
    entry!(
        "Claude 2",
        Retired,
        Some("claude-sonnet-5"),
        ["claude-2", "claude-2-0", "claude-2-1"]
    ),
    entry!("Claude 3", Retired, Some("claude-sonnet-5"), ["claude-3"]),
    entry!(
        "Claude 3 Opus",
        Retired,
        Some("claude-opus-5"),
        ["claude-3-opus", "claude-opus-3"]
    ),
    entry!(
        "Claude 3 Sonnet",
        Retired,
        Some("claude-sonnet-5"),
        ["claude-3-sonnet", "claude-sonnet-3"]
    ),
    entry!(
        "Claude 3 Haiku",
        Retired,
        Some("claude-haiku-4-5"),
        ["claude-3-haiku", "claude-haiku-3"]
    ),
    entry!(
        "Claude 3.5",
        Retired,
        Some("claude-sonnet-5"),
        ["claude-3-5"]
    ),
    entry!(
        "Claude 3.5 Sonnet",
        Retired,
        Some("claude-sonnet-5"),
        ["claude-3-5-sonnet", "claude-sonnet-3-5"]
    ),
    entry!(
        "Claude 3.5 Haiku",
        Retired,
        Some("claude-haiku-4-5"),
        ["claude-3-5-haiku", "claude-haiku-3-5"]
    ),
    entry!(
        "Claude 3.7 Sonnet",
        Retired,
        Some("claude-sonnet-5"),
        ["claude-3-7", "claude-3-7-sonnet", "claude-sonnet-3-7"]
    ),
    entry!(
        "Claude Sonnet 4",
        Deprecated,
        Some("claude-sonnet-5"),
        [
            "claude-sonnet-4",
            "claude-sonnet-4-0",
            "claude-4-sonnet",
            "claude-4-0-sonnet"
        ]
    ),
    entry!(
        "Claude Opus 4",
        Deprecated,
        Some("claude-opus-5"),
        [
            "claude-opus-4",
            "claude-opus-4-0",
            "claude-4-opus",
            "claude-4-0-opus"
        ]
    ),
    entry!(
        "Claude Opus 4.1",
        Retired,
        Some("claude-opus-5"),
        ["claude-opus-4-1", "claude-4-1-opus"]
    ),
    entry!(
        "Claude Sonnet 4.5",
        Superseded,
        Some("claude-sonnet-5"),
        ["claude-sonnet-4-5", "claude-4-5-sonnet"]
    ),
    entry!(
        "Claude Opus 4.5",
        Superseded,
        Some("claude-opus-5"),
        ["claude-opus-4-5", "claude-4-5-opus"]
    ),
    entry!(
        "Claude Haiku 4.5",
        Current,
        None,
        ["claude-haiku-4-5", "claude-4-5-haiku"]
    ),
    entry!(
        "Claude Sonnet 4.6",
        Current,
        None,
        ["claude-sonnet-4-6", "claude-4-6-sonnet"]
    ),
    entry!(
        "Claude Opus 4.6",
        Current,
        None,
        ["claude-opus-4-6", "claude-4-6-opus"]
    ),
    entry!(
        "Claude Opus 4.7",
        Current,
        None,
        ["claude-opus-4-7", "claude-4-7-opus"]
    ),
    entry!(
        "Claude Opus 4.8",
        Current,
        None,
        ["claude-opus-4-8", "claude-4-8-opus"]
    ),
    entry!(
        "Claude Opus 5",
        Current,
        None,
        ["claude-opus-5", "claude-5-opus"]
    ),
    entry!(
        "Claude Sonnet 5",
        Current,
        None,
        ["claude-sonnet-5", "claude-5-sonnet"]
    ),
    entry!(
        "Claude Fable 5",
        Current,
        None,
        ["claude-fable-5", "claude-5-fable"]
    ),
    entry!(
        "Claude Fable 5.1",
        Current,
        None,
        ["claude-fable-5-1", "claude-5-1-fable"]
    ),
    entry!(
        "Claude Mythos 5",
        Current,
        None,
        ["claude-mythos-5", "claude-5-mythos"]
    ),
    entry!(
        "Claude Mythos 5.1",
        Current,
        None,
        ["claude-mythos-5-1", "claude-5-1-mythos"]
    ),
    // ── OpenAI ──────────────────────────────────────────────────────────
    entry!("text-davinci", Retired, Some("gpt-5"), ["text-davinci"]),
    entry!("code-davinci", Retired, Some("gpt-5"), ["code-davinci"]),
    entry!(
        "GPT-3.5",
        Superseded,
        Some("gpt-5-mini"),
        [
            "gpt-3-5",
            "gpt-3-5-turbo",
            "gpt-3-5-turbo-16k",
            "gpt-35-turbo"
        ]
    ),
    entry!(
        "GPT-4 Turbo",
        Superseded,
        Some("gpt-5"),
        [
            "gpt-4-turbo",
            "gpt-4-turbo-preview",
            "gpt-4-1106-preview",
            "gpt-4-0125-preview"
        ]
    ),
    entry!("GPT-4 32k", Retired, Some("gpt-5"), ["gpt-4-32k"]),
    entry!(
        "GPT-4 Vision Preview",
        Retired,
        Some("gpt-5"),
        ["gpt-4-vision-preview", "gpt-4-vision"]
    ),
    entry!(
        "GPT-4.5 Preview",
        Retired,
        Some("gpt-5"),
        ["gpt-4-5", "gpt-4-5-preview"]
    ),
    entry!("GPT-4o", Superseded, Some("gpt-5"), ["gpt-4o"]),
    entry!(
        "GPT-4o mini",
        Superseded,
        Some("gpt-5-mini"),
        ["gpt-4o-mini"]
    ),
    entry!("GPT-4.1", Superseded, Some("gpt-5"), ["gpt-4-1"]),
    entry!(
        "GPT-4.1 mini",
        Superseded,
        Some("gpt-5-mini"),
        ["gpt-4-1-mini"]
    ),
    entry!(
        "GPT-4.1 nano",
        Superseded,
        Some("gpt-5-nano"),
        ["gpt-4-1-nano"]
    ),
    entry!("o1-preview", Retired, Some("gpt-5"), ["o1-preview"]),
    entry!("o1-mini", Retired, Some("gpt-5-mini"), ["o1-mini"]),
    entry!("o1-pro", Superseded, Some("gpt-5-pro"), ["o1-pro"]),
    entry!("o3-mini", Superseded, Some("gpt-5-mini"), ["o3-mini"]),
    entry!("o3-pro", Superseded, Some("gpt-5-pro"), ["o3-pro"]),
    entry!("o4-mini", Superseded, Some("gpt-5-mini"), ["o4-mini"]),
    entry!(
        "GPT-5",
        Current,
        None,
        [
            "gpt-5",
            "gpt-5-mini",
            "gpt-5-nano",
            "gpt-5-pro",
            "gpt-5-codex"
        ]
    ),
    // ── Google ──────────────────────────────────────────────────────────
    entry!(
        "PaLM 2",
        Retired,
        Some("gemini-2-5-pro"),
        ["palm-2", "text-bison", "chat-bison"]
    ),
    entry!(
        "Gemini 1.0",
        Retired,
        Some("gemini-2-5-pro"),
        [
            "gemini-pro",
            "gemini-pro-vision",
            "gemini-ultra",
            "gemini-1-0",
            "gemini-1-0-pro",
            "gemini-1-0-ultra"
        ]
    ),
    entry!(
        "Gemini 1.5",
        Retired,
        Some("gemini-2-5-flash"),
        ["gemini-1-5", "gemini-1-5-pro", "gemini-1-5-flash"]
    ),
    entry!(
        "Gemini 2.0",
        Superseded,
        Some("gemini-2-5-flash"),
        [
            "gemini-2-0",
            "gemini-2-0-flash",
            "gemini-2-0-flash-lite",
            "gemini-2-0-pro"
        ]
    ),
    entry!(
        "Gemini 2.5",
        Current,
        None,
        [
            "gemini-2-5",
            "gemini-2-5-pro",
            "gemini-2-5-flash",
            "gemini-2-5-flash-lite"
        ]
    ),
    entry!(
        "Gemini 3",
        Current,
        None,
        ["gemini-3", "gemini-3-pro", "gemini-3-flash"]
    ),
    // ── Meta ────────────────────────────────────────────────────────────
    entry!("Llama 2", Superseded, Some("llama-4"), ["llama-2"]),
    entry!(
        "Llama 3",
        Current,
        None,
        ["llama-3", "llama-3-1", "llama-3-2", "llama-3-3"]
    ),
    entry!("Llama 4", Current, None, ["llama-4"]),
];

static BUILTIN_INDEX: LazyLock<HashMap<&'static str, &'static ModelEntry>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for entry in BUILTIN {
        for key in entry.keys {
            let prev = map.insert(*key, entry);
            debug_assert!(prev.is_none(), "duplicate catalog key {key}");
        }
    }
    map
});

/// Finds candidate model mentions: a vendor word followed by version numbers
/// and a closed set of qualifier words, joined by `-`, `_`, `.`, or a space.
static TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:claude|gpt|gemini|llama|palm|text-davinci|code-davinci|text-bison|chat-bison|o[134]|opus|sonnet|haiku)",
        r"(?:[-_ .]?(?:\d+[a-z]?|v\d+|opus|sonnet|haiku|instant|fable|mythos|turbo|mini|nano|pro|flash|lite|ultra|preview|vision|latest|codex|thinking))*\b",
    ))
    .unwrap()
});

/// Everything the checker needs to know about one model mention.
pub(crate) struct Classification {
    pub key: String,
    /// `None` when the model is not in the catalog.
    pub status: Option<ModelStatus>,
    pub name: Option<&'static str>,
    pub successor: Option<&'static str>,
    /// Trailing snapshot date (`YYYYMMDD` or `YYYY-MM-DD`) as `(year, month, day)`.
    pub snapshot: Option<Snapshot>,
}

pub(crate) struct ModelCatalog {
    user_current: HashSet<String>,
}

impl ModelCatalog {
    /// `current_models` are user-supplied names that must never be flagged.
    pub(crate) fn new(current_models: &[String]) -> Self {
        Self {
            user_current: current_models
                .iter()
                .filter_map(|m| normalize(m).map(|(parts, _)| parts.join("-")))
                .collect(),
        }
    }

    /// Yield `(byte_start, byte_end, text)` for each candidate model mention in `text`.
    pub(crate) fn mentions<'t>(
        &self,
        text: &'t str,
    ) -> impl Iterator<Item = (usize, usize, &'t str)> {
        TOKEN
            .find_iter(text)
            .map(|m| (m.start(), m.end(), m.as_str()))
    }

    /// Classify one mention. Returns `None` for text that carries no version
    /// information at all (a bare "Claude" or "GPT").
    pub(crate) fn classify(&self, mention: &str) -> Option<Classification> {
        let (mut parts, snapshot) = normalize(mention)?;
        // Exact lookup first; then peel trailing qualifier words, parameter
        // sizes, and revisions (never plain version numbers) so
        // `gemini-2-5-flash-preview` resolves to `gemini-2-5-flash` while an
        // unknown `claude-opus-4-9` stays unknown. User-declared current
        // models win at every step.
        let mut key = parts.join("-");
        let entry = loop {
            if self.user_current.contains(&key) {
                return Some(Classification {
                    key,
                    status: Some(Current),
                    name: None,
                    successor: None,
                    snapshot,
                });
            }
            if let Some(entry) = BUILTIN_INDEX.get(key.as_str()) {
                break Some(*entry);
            }
            if parts.len() <= 2 || !parts.last().is_some_and(|p| is_peelable(p)) {
                break None;
            }
            parts.pop();
            key = parts.join("-");
        };
        Some(Classification {
            key,
            status: entry.map(|e| e.status),
            name: entry.map(|e| e.name),
            successor: entry.and_then(|e| e.successor),
            snapshot,
        })
    }
}

/// Vendor words that are sometimes written glued to a version (`gpt4o`, `claude2`).
const GLUED_VENDORS: [&str; 5] = ["claude", "gpt", "gemini", "llama", "palm"];

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// A parameter-count suffix such as `70b`.
fn is_param_size(s: &str) -> bool {
    s.len() >= 2 && s.ends_with('b') && is_digits(&s[..s.len() - 1])
}

/// A revision suffix that never carries version meaning: `0125`, `002`, `v2`.
fn is_revision(s: &str) -> bool {
    (is_digits(s) && (3..=4).contains(&s.len()))
        || (s.len() >= 2 && s.starts_with('v') && is_digits(&s[1..]))
}

/// Trailing parts that may be dropped while searching for a catalog match.
fn is_peelable(s: &str) -> bool {
    is_param_size(s) || is_revision(s) || s.chars().all(|c| c.is_ascii_alphabetic())
}

/// Split a mention into lowercase parts and strip trailing snapshot dates,
/// `latest`, parameter sizes (`70b`), and revisions (`0125`, `-v2`).
/// Family-first spellings without a vendor (`Sonnet 4.5`) get `claude`
/// prepended and glued spellings (`GPT3.5`) are split. Returns `None` when
/// no version information remains.
fn normalize(mention: &str) -> Option<(Vec<String>, Option<Snapshot>)> {
    let mut parts: Vec<String> = mention
        .split(['-', '_', '.', ' '])
        .filter(|p| !p.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();

    let first = parts.first()?.clone();
    if let Some(at) = first.find(|c: char| c.is_ascii_digit()) {
        if at > 0 && GLUED_VENDORS.contains(&&first[..at]) {
            parts[0].truncate(at);
            parts.insert(1, first[at..].to_string());
        }
    }
    if matches!(parts[0].as_str(), "opus" | "sonnet" | "haiku") {
        if !parts
            .get(1)
            .is_some_and(|p| p.starts_with(|c: char| c.is_ascii_digit()))
        {
            return None;
        }
        parts.insert(0, "claude".to_string());
    }
    // "Claude 3 times", "Claude 2 weeks ago": a vendor and a bare integer
    // joined by a space is prose. Hyphenated (`claude-3`) and dotted
    // (`Claude 3.5`) forms still count.
    if parts.len() == 2 && mention.contains(' ') && is_digits(&parts[1]) {
        return None;
    }

    let mut snapshot = None;
    while parts.len() >= 2 {
        let n = parts.len();
        let last = parts[n - 1].as_str();
        if last == "latest" || is_param_size(last) || (n > 2 && is_revision(last)) {
            parts.pop();
        } else if last.len() == 8 && is_digits(last) {
            snapshot = parse_ymd(&last[..4], &last[4..6], &last[6..]);
            parts.pop();
        } else if n >= 4
            && last.len() == 2
            && is_digits(last)
            && parts[n - 2].len() == 2
            && is_digits(&parts[n - 2])
            && parts[n - 3].len() == 4
            && is_digits(&parts[n - 3])
        {
            snapshot = parse_ymd(&parts[n - 3], &parts[n - 2], last);
            parts.truncate(n - 3);
        } else {
            break;
        }
    }
    (parts.len() >= 2 || snapshot.is_some()).then_some((parts, snapshot))
}

fn parse_ymd(y: &str, m: &str, d: &str) -> Option<Snapshot> {
    let (y, m, d): (i64, u32, u32) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    ((2015..=2100).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d))
        .then_some((y, m, d))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_of(s: &str) -> Option<String> {
        normalize(s).map(|(p, _)| p.join("-"))
    }

    fn status_of(s: &str) -> Option<ModelStatus> {
        ModelCatalog::new(&[]).classify(s).and_then(|c| c.status)
    }

    #[test]
    fn normalizes_prose_and_id_spellings_to_same_key() {
        for s in [
            "Claude 3.5 Sonnet",
            "claude-3-5-sonnet-20241022",
            "claude-3-5-sonnet-latest",
            "claude_3_5_sonnet",
            "CLAUDE-3.5-SONNET",
        ] {
            assert_eq!(key_of(s).as_deref(), Some("claude-3-5-sonnet"), "{s}");
        }
        assert_eq!(
            key_of("Claude Sonnet 3.5").as_deref(),
            Some("claude-sonnet-3-5")
        );
        assert_eq!(key_of("Sonnet 4.5").as_deref(), Some("claude-sonnet-4-5"));
        assert_eq!(key_of("gpt-4o-2024-08-06").as_deref(), Some("gpt-4o"));
        assert_eq!(key_of("Llama 2 70B").as_deref(), Some("llama-2"));
        assert_eq!(
            key_of("claude-3-5-sonnet-20241022-v2").as_deref(),
            Some("claude-3-5-sonnet")
        );
    }

    #[test]
    fn bare_vendor_or_family_word_is_not_a_model() {
        assert!(key_of("Claude").is_none());
        assert!(key_of("gpt").is_none());
        assert!(key_of("sonnet").is_none());
        assert!(key_of("opus magnum").is_none());
        assert!(key_of("claude-3-5-sonnet-20241022").is_some());
    }

    #[test]
    fn extracts_snapshot_dates_in_both_formats() {
        assert_eq!(
            normalize("claude-sonnet-4-20250514").unwrap().1,
            Some((2025, 5, 14))
        );
        assert_eq!(
            normalize("gpt-4o-2024-08-06").unwrap().1,
            Some((2024, 8, 6))
        );
        assert_eq!(normalize("claude-sonnet-4-5").unwrap().1, None);
        // Invalid calendar values are not dates.
        assert_eq!(normalize("claude-foo-20251399").unwrap().1, None);
    }

    #[test]
    fn statuses_follow_catalog() {
        assert_eq!(status_of("claude-3-5-sonnet-20241022"), Some(Retired));
        assert_eq!(status_of("Claude Sonnet 3.5"), Some(Retired));
        assert_eq!(status_of("claude-sonnet-4-20250514"), Some(Deprecated));
        assert_eq!(status_of("Claude Sonnet 4.5"), Some(Superseded));
        assert_eq!(status_of("claude-sonnet-4-6"), Some(Current));
        assert_eq!(status_of("claude-opus-5"), Some(Current));
        assert_eq!(status_of("gpt-4o"), Some(Superseded));
        assert_eq!(status_of("GPT-3.5 Turbo"), Some(Superseded));
        assert_eq!(status_of("text-davinci-003"), Some(Retired));
        assert_eq!(status_of("o3-mini"), Some(Superseded));
        assert_eq!(status_of("gemini-1.5-pro"), Some(Retired));
    }

    #[test]
    fn unknown_versions_are_not_guessed() {
        // A future minor version must not collapse onto its major's verdict.
        assert_eq!(status_of("claude-opus-4-9"), None);
        assert_eq!(status_of("gpt-4"), None);
        assert_eq!(status_of("o1"), None);
        assert_eq!(status_of("gemini-4-pro"), None);
    }

    #[test]
    fn trailing_qualifier_words_are_peeled_but_numbers_are_not() {
        assert_eq!(status_of("gemini-2-5-flash-preview"), Some(Current));
        assert_eq!(status_of("gpt-4-turbo-preview"), Some(Superseded));
        assert_eq!(status_of("claude-4-9-opus"), None);
    }

    #[test]
    fn user_current_models_override_catalog() {
        let catalog = ModelCatalog::new(&["gpt-4o".to_string(), "Claude Sonnet 4".to_string()]);
        assert_eq!(catalog.classify("GPT-4o").unwrap().status, Some(Current));
        assert_eq!(
            catalog.classify("claude-sonnet-4-20250514").unwrap().status,
            Some(Current)
        );
        assert_eq!(
            catalog.classify("gpt-4o-mini").unwrap().status,
            Some(Superseded)
        );
    }

    #[test]
    fn mentions_stop_at_unknown_words() {
        let catalog = ModelCatalog::new(&[]);
        let found: Vec<&str> = catalog
            .mentions("Use Claude 3.5 Sonnet for summaries, gpt-4o for chat.")
            .map(|(_, _, t)| t)
            .collect();
        assert_eq!(found, vec!["Claude 3.5 Sonnet", "gpt-4o"]);
        let found: Vec<&str> = catalog
            .mentions("Claude Code reads CLAUDE.md; Claude's memory")
            .map(|(_, _, t)| t)
            .collect();
        assert!(found.iter().all(|t| key_of(t).is_none()), "{found:?}");
    }

    #[test]
    fn empty_or_separator_only_input_is_none() {
        for s in ["", " ", "-", "...", "_-_"] {
            assert!(normalize(s).is_none(), "{s:?}");
        }
        // A blank config entry must not panic the run.
        let catalog = ModelCatalog::new(&[String::new(), "-".to_string()]);
        assert_eq!(catalog.classify("gpt-4o").unwrap().status, Some(Superseded));
    }

    #[test]
    fn bare_vendor_and_integer_joined_by_space_is_prose() {
        assert!(key_of("Claude 3").is_none());
        assert!(key_of("Claude 2").is_none());
        assert_eq!(key_of("claude-3").as_deref(), Some("claude-3"));
        assert_eq!(key_of("Claude 3.5").as_deref(), Some("claude-3-5"));
        assert_eq!(key_of("Claude 3 Opus").as_deref(), Some("claude-3-opus"));
    }

    #[test]
    fn glued_vendor_and_version_are_split() {
        assert_eq!(key_of("GPT3.5").as_deref(), Some("gpt-3-5"));
        assert_eq!(key_of("claude2").as_deref(), Some("claude-2"));
        assert_eq!(key_of("gpt4o").as_deref(), Some("gpt-4o"));
        assert_eq!(key_of("Claude3 Sonnet").as_deref(), Some("claude-3-sonnet"));
        assert_eq!(key_of("GPT4 Turbo").as_deref(), Some("gpt-4-turbo"));
        assert!(key_of("o1").is_none());
    }

    #[test]
    fn revisions_and_sizes_are_stripped_but_versions_kept() {
        assert_eq!(status_of("gpt-3.5-turbo-0125"), Some(Superseded));
        assert_eq!(status_of("gpt-3.5-turbo-1106"), Some(Superseded));
        assert_eq!(status_of("gemini-1.5-pro-002"), Some(Retired));
        assert_eq!(status_of("claude-3-5-sonnet-v2"), Some(Retired));
        assert_eq!(status_of("llama-2-13b-chat"), Some(Superseded));
        assert_eq!(status_of("claude-v1"), Some(Retired));
        assert_eq!(status_of("claude-1-3"), Some(Retired));
        assert_eq!(status_of("gpt-4-32k-0613"), Some(Retired));
        assert_eq!(status_of("claude-3-5"), Some(Retired));
    }

    #[test]
    fn snapshot_only_mentions_keep_their_date() {
        let (parts, snapshot) = normalize("o3-2025-04-16").unwrap();
        assert_eq!(parts, vec!["o3"]);
        assert_eq!(snapshot, Some((2025, 4, 16)));
        let class = ModelCatalog::new(&[]).classify("o3-2025-04-16").unwrap();
        assert_eq!(class.status, None);
        assert_eq!(class.snapshot, Some((2025, 4, 16)));
    }

    #[test]
    fn user_current_models_apply_after_peeling() {
        let catalog = ModelCatalog::new(&[
            "gemini-2.0-flash".to_string(),
            "claude-sonnet-4".to_string(),
        ]);
        assert_eq!(
            catalog
                .classify("gemini-2.0-flash-thinking")
                .unwrap()
                .status,
            Some(Current)
        );
        assert_eq!(
            catalog.classify("claude-sonnet-4-thinking").unwrap().status,
            Some(Current)
        );
        assert_eq!(
            catalog.classify("gemini-2.0-pro").unwrap().status,
            Some(Superseded)
        );
    }

    #[test]
    fn catalog_keys_are_unique_and_normalized() {
        let mut seen = HashSet::new();
        for entry in BUILTIN {
            for key in entry.keys {
                assert!(seen.insert(*key), "duplicate key {key}");
                assert_eq!(
                    key_of(key).as_deref(),
                    Some(*key),
                    "key {key} is not in normalized form"
                );
            }
        }
    }
}
