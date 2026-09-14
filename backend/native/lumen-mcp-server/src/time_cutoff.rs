//! `time_cutoff` parsing and serialization.
//!
//! The Python tool parses the caller's string with
//! `TypeAdapter(datetime | None)` and hands the result to Pydantic, which
//! serializes it back into the `/search` request body. Both halves are
//! reproduced here; the behaviour was measured against Pydantic v2 rather than
//! assumed.
//!
//! Parsing accepts: RFC 3339 with `Z` or an offset, a naive ISO timestamp, a
//! date on its own, a space-separated timestamp, and a Unix timestamp (read as
//! UTC). Anything else fails, and the caller drops the filter rather than
//! erroring, exactly as the Python tool does.

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc};

/// A parsed cutoff. Pydantic keeps naive and aware values apart, and serializes
/// them differently, so this does too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeCutoff {
    Aware(DateTime<FixedOffset>),
    Naive(NaiveDateTime),
}

const NAIVE_FORMATS: [&str; 4] = [
    "%Y-%m-%dT%H:%M:%S%.f",
    "%Y-%m-%d %H:%M:%S%.f",
    "%Y-%m-%dT%H:%M",
    "%Y-%m-%d %H:%M",
];

impl TimeCutoff {
    /// Parse a caller-supplied string. `None` means the value was unusable.
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }

        if let Ok(parsed) = DateTime::parse_from_rfc3339(raw) {
            return Some(Self::Aware(parsed));
        }

        for format in NAIVE_FORMATS {
            if let Ok(parsed) = NaiveDateTime::parse_from_str(raw, format) {
                return Some(Self::Naive(parsed));
            }
        }

        if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
            // Pydantic widens a bare date to midnight, with no timezone.
            return Some(Self::Naive(date.and_hms_opt(0, 0, 0)?));
        }

        // A bare integer or float is a Unix timestamp, read as UTC.
        if let Ok(seconds) = raw.parse::<i64>() {
            return unix_seconds(seconds as f64);
        }
        if let Ok(seconds) = raw.parse::<f64>() {
            return unix_seconds(seconds);
        }

        None
    }

    /// Render the way Pydantic's JSON serializer does.
    ///
    /// Fractional seconds appear as exactly six digits when microseconds are
    /// non-zero, and are omitted otherwise. A UTC offset renders as `Z`.
    pub fn to_pydantic_json(&self) -> String {
        match self {
            Self::Naive(value) => format_naive(value),
            Self::Aware(value) => {
                let base = format_naive(&value.naive_local());
                if value.offset().local_minus_utc() == 0 {
                    format!("{base}Z")
                } else {
                    format!("{base}{}", value.format("%:z"))
                }
            }
        }
    }
}

fn unix_seconds(seconds: f64) -> Option<TimeCutoff> {
    let whole = seconds.trunc() as i64;
    let nanos = ((seconds - seconds.trunc()) * 1e9).round() as u32;
    let utc = Utc.timestamp_opt(whole, nanos).single()?;
    Some(TimeCutoff::Aware(utc.into()))
}

fn format_naive(value: &NaiveDateTime) -> String {
    if value.and_utc().timestamp_subsec_micros() == 0 {
        value.format("%Y-%m-%dT%H:%M:%S").to_string()
    } else {
        value.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every expectation here was produced by running the value through
    /// Pydantic v2 in this repo's virtualenv.
    #[test]
    fn round_trips_match_pydantic() {
        let cases = [
            ("2025-11-24T00:00:00Z", "2025-11-24T00:00:00Z"),
            ("2025-11-24T00:00:00+02:00", "2025-11-24T00:00:00+02:00"),
            ("2025-11-24T00:00:00", "2025-11-24T00:00:00"),
            ("2025-11-24", "2025-11-24T00:00:00"),
            ("2025-11-24T12:34:56.789Z", "2025-11-24T12:34:56.789000Z"),
            ("2025-11-24 12:34:56", "2025-11-24T12:34:56"),
            ("1732406400", "2024-11-24T00:00:00Z"),
            ("2025-11-24T00:00:00.000000Z", "2025-11-24T00:00:00Z"),
        ];

        for (input, expected) in cases {
            let parsed =
                TimeCutoff::parse(input).unwrap_or_else(|| panic!("{input:?} should parse"));
            assert_eq!(parsed.to_pydantic_json(), expected, "input {input:?}");
        }
    }

    #[test]
    fn unusable_values_yield_none() {
        for input in ["not-a-date", "", "   ", "2025-13-45", "tomorrow"] {
            assert_eq!(TimeCutoff::parse(input), None, "input {input:?}");
        }
    }

    #[test]
    fn naive_and_aware_stay_distinct() {
        assert!(matches!(
            TimeCutoff::parse("2025-11-24T00:00:00"),
            Some(TimeCutoff::Naive(_))
        ));
        assert!(matches!(
            TimeCutoff::parse("2025-11-24T00:00:00Z"),
            Some(TimeCutoff::Aware(_))
        ));
    }
}
