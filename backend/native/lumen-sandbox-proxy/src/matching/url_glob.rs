//! URL globs, as authored by admins for CUSTOM apps.
//!
//! Port of `backend/lumen/external_apps/url_glob.py`. Built-in providers author
//! regexes directly; only CUSTOM apps go through a glob, and the host half of
//! one must stay literal because it is the credential-injection boundary.

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UrlGlobError {
    #[error("URL pattern must not be empty.")]
    Empty,
    #[error("URL pattern must start with http:// or https://: {value:?}")]
    MissingScheme { value: String },
    #[error("URL pattern must include a host: {value:?}")]
    MissingHost { value: String },
    #[error("URL pattern host must be literal (no wildcards): {value:?}")]
    WildcardHost { value: String },
}

/// A validated URL glob. Build one with [`UrlGlob::parse`] at trust boundaries;
/// [`UrlGlob::trusted`] skips validation for values already stored in the
/// database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlGlob {
    value: String,
}

/// The length of a leading `http://` or `https://`, case-insensitively, or
/// `None` when there is not one.
fn scheme_length(value: &str) -> Option<usize> {
    for prefix in ["https://", "http://"] {
        // `get` rather than a slice: a multi-byte character straddling the
        // boundary would panic on `&value[..n]`.
        if value
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        {
            return Some(prefix.len());
        }
    }
    None
}

impl UrlGlob {
    /// Validate and build. Rejects anything whose host is not literal.
    pub fn parse(value: &str) -> Result<Self, UrlGlobError> {
        let stripped = value.trim();
        if stripped.is_empty() {
            return Err(UrlGlobError::Empty);
        }
        let Some(scheme_end) = scheme_length(stripped) else {
            return Err(UrlGlobError::MissingScheme {
                value: stripped.to_string(),
            });
        };

        // The host (up to the first path separator) is the credential-injection
        // boundary, so it must be literal.
        let host = stripped[scheme_end..].split('/').next().unwrap_or("");
        if host.is_empty() {
            return Err(UrlGlobError::MissingHost {
                value: stripped.to_string(),
            });
        }
        if host.contains('*') {
            return Err(UrlGlobError::WildcardHost {
                value: stripped.to_string(),
            });
        }
        Ok(Self {
            value: stripped.to_string(),
        })
    }

    /// Wrap an already-validated value, as loaded from the database.
    pub fn trusted(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    /// The regex this glob matches as, anchored so it is a full match.
    ///
    /// `*` becomes `.*`; every other character is literal. `\A`/`\z` make it a
    /// full match, which is what Python's `re.fullmatch` gives the caller.
    pub fn to_regex(&self) -> String {
        let body = self
            .value
            .split('*')
            .map(regex::escape)
            .collect::<Vec<_>>()
            .join(".*");
        format!(r"\A(?:{body})\z")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(glob: &str, url: &str) -> bool {
        let pattern = UrlGlob::parse(glob).expect("a valid glob").to_regex();
        regex::Regex::new(&pattern)
            .expect("a compilable regex")
            .is_match(url)
    }

    #[test]
    fn a_glob_matches_the_whole_url_or_nothing() {
        assert!(matches(
            "https://api.example.com/v1/*",
            "https://api.example.com/v1/users"
        ));
        assert!(!matches(
            "https://api.example.com/v1/*",
            "https://api.example.com/v2/users"
        ));
        // A full match, not a prefix one: a longer host must not sneak in.
        assert!(!matches(
            "https://api.example.com/v1/*",
            "https://evil.com/?x=https://api.example.com/v1/users"
        ));
    }

    #[test]
    fn regex_metacharacters_in_a_glob_are_literal() {
        assert!(matches(
            "https://api.example.com/a.b",
            "https://api.example.com/a.b"
        ));
        // `.` must not match an arbitrary character.
        assert!(!matches(
            "https://api.example.com/a.b",
            "https://api.example.com/axb"
        ));
    }

    #[test]
    fn a_wildcard_does_not_cross_a_newline() {
        // Same as Python's `.`: a body smuggling a newline cannot widen a glob.
        assert!(!matches(
            "https://api.example.com/*",
            "https://api.example.com/a\nhttps://evil.com/b"
        ));
    }

    #[test]
    fn a_host_must_be_present_literal_and_schemed() {
        assert_eq!(UrlGlob::parse("   "), Err(UrlGlobError::Empty));
        assert!(matches!(
            UrlGlob::parse("api.example.com/*"),
            Err(UrlGlobError::MissingScheme { .. })
        ));
        assert!(matches!(
            UrlGlob::parse("https:///path"),
            Err(UrlGlobError::MissingHost { .. })
        ));
        // The one that matters: a wildcard host would send credentials anywhere.
        assert!(matches!(
            UrlGlob::parse("https://*.example.com/v1/*"),
            Err(UrlGlobError::WildcardHost { .. })
        ));
    }

    #[test]
    fn a_scheme_is_recognised_case_insensitively() {
        assert!(UrlGlob::parse("HTTPS://api.example.com/*").is_ok());
        assert!(UrlGlob::parse("HtTp://api.example.com/*").is_ok());
    }

    #[test]
    fn surrounding_whitespace_is_trimmed_before_validation() {
        let glob = UrlGlob::parse("  https://api.example.com/v1/*  ").unwrap();
        assert_eq!(glob.value(), "https://api.example.com/v1/*");
    }
}
