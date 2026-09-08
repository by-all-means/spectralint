//! Calendar helpers shared by time-aware checkers and the result cache.
//!
//! "Today" honours the `SPECTRALINT_CURRENT_DATE` environment variable
//! (`YYYY-MM-DD` or `YYYY-MM`) so CI runs and tests can be reproducible;
//! otherwise it is read from the system clock (UTC) on every call, so
//! long-running modes (`--watch`, the LSP server) see the date advance.

use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const ENV_CURRENT_DATE: &str = "SPECTRALINT_CURRENT_DATE";

static WARNED_MALFORMED: Once = Once::new();

/// `YYYY-MM-DD` or `YYYY-MM`; an unparsable day segment falls back to the 1st.
fn parse_override(value: &str) -> Option<(u32, u32, u32)> {
    let mut parts = value.trim().split('-');
    let y: u32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(1);
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}

/// Today's civil date as `(year, month, day)`.
pub(crate) fn today() -> (u32, u32, u32) {
    if let Ok(value) = std::env::var(ENV_CURRENT_DATE) {
        match parse_override(&value) {
            Some(date) => return date,
            None => WARNED_MALFORMED.call_once(|| {
                tracing::warn!(
                    "Ignoring malformed {ENV_CURRENT_DATE}={value:?}; expected YYYY-MM-DD or YYYY-MM"
                );
            }),
        }
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    (y as u32, m, d)
}

/// Today as a day count since 1970-01-01.
pub(crate) fn today_days() -> i64 {
    let (y, m, d) = today();
    days_from_civil(i64::from(y), m, d)
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
pub(crate) fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
pub(crate) fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_is_day_zero() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn known_dates_roundtrip() {
        for (y, m, d) in [(2000, 2, 29), (2024, 12, 31), (2025, 5, 14), (2026, 9, 7)] {
            let days = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(days), (y, m, d));
        }
        // 2025-05-14 is 20222 days after the epoch.
        assert_eq!(days_from_civil(2025, 5, 14), 20_222);
    }

    #[test]
    fn override_accepts_full_month_and_sloppy_day_forms() {
        assert_eq!(parse_override("2026-09-07"), Some((2026, 9, 7)));
        assert_eq!(parse_override("2026-03"), Some((2026, 3, 1)));
        assert_eq!(parse_override("2026-03-xx"), Some((2026, 3, 1)));
        assert_eq!(parse_override("2026"), None);
        assert_eq!(parse_override("garbage"), None);
        assert_eq!(parse_override("2026-13-01"), None);
    }

    #[test]
    fn today_is_plausible() {
        let (y, m, d) = today();
        assert!(y >= 2026, "clock says {y}-{m}-{d}");
        assert!((1..=12).contains(&m) && (1..=31).contains(&d));
    }
}
