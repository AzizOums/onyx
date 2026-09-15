//! The recognition rules a catalog action is defined by.
//!
//! Port of `backend/lumen/external_apps/providers/actions.py`.

use serde::{Deserialize, Serialize};

use super::policy::EndpointPolicy;

/// Recognises a REST request as an action by HTTP method + path template.
///
/// `path` is compared segment-by-segment (see [`path_matches`]): a `{name}`
/// segment matches exactly one path segment, a trailing `{name...}` segment
/// matches one or more remaining segments, and literal segments must match
/// verbatim. Placeholders name the resource for readability; the decision uses
/// only the matched action, never the captured value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestRoute {
    pub method: String,
    pub path: String,
}

/// Recognises a GraphQL request by operation type + root field. The URL is
/// identical for every operation, so the body is the only discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphQlOp {
    pub operation_type: String,
    pub field: String,
}

/// A request matches an action when any of the action's rules fires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum MatchRule {
    #[serde(rename = "rest")]
    Rest(RestRoute),
    #[serde(rename = "graphql")]
    GraphQl(GraphQlOp),
}

/// One logical action a provider can take: a stable id bound to its admin
/// display strings and its recognition rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointSpec {
    pub id: String,
    pub normalised_name: String,
    pub description: String,
    pub matches: Vec<MatchRule>,
    /// The policy a freshly-created app starts this action at, unless an admin
    /// overrides it.
    pub default_policy: EndpointPolicy,
}

/// A catalog entry the Python side rejects at load time.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuleError {
    #[error("'{{name...}}' wildcard must be the last segment: {path:?}")]
    NonTrailingWildcard { path: String },
}

impl RestRoute {
    /// The check Python runs as a Pydantic field validator.
    ///
    /// [`path_matches`] only honours a `{name...}` wildcard in the last
    /// segment, so a catalog that puts one elsewhere is rejected at load time
    /// rather than silently mis-matching every request.
    pub fn validate(&self) -> Result<(), RuleError> {
        let trimmed = self.path.trim_end_matches('/');
        let segments: Vec<&str> = trimmed.split('/').collect();
        let leading = &segments[..segments.len().saturating_sub(1)];
        if leading.iter().any(|segment| segment.ends_with("...}")) {
            return Err(RuleError::NonTrailingWildcard {
                path: self.path.clone(),
            });
        }
        Ok(())
    }
}

/// One template segment against one path segment: a `{name}` placeholder
/// matches any single non-empty segment, a literal must match exactly.
fn segment_matches(expected: &str, actual: &str) -> bool {
    if expected.starts_with('{') && expected.ends_with('}') {
        return !actual.is_empty(); // a placeholder requires a non-empty segment
    }
    expected == actual
}

/// Whether a request `path` matches a [`RestRoute::path`] template.
///
/// Segments (split on `/`) are compared positionally. A trailing `{name...}`
/// segment matches one *or more* remaining segments, for tails that are
/// themselves slash-bearing (a `contents` file path, a `heads/feature/x` ref).
/// Trailing slashes on either side are ignored.
pub fn path_matches(template: &str, path: &str) -> bool {
    let expected: Vec<&str> = template.trim_end_matches('/').split('/').collect();
    let actual: Vec<&str> = path.trim_end_matches('/').split('/').collect();

    // `split` on a non-empty separator always yields at least one element, so
    // `last` is infallible here — same as Python's `expected_segments[-1]`.
    let last = expected[expected.len() - 1];
    if last.starts_with('{') && last.ends_with("...}") {
        let prefix = &expected[..expected.len() - 1];
        if actual.len() <= prefix.len() {
            return false; // the wildcard must swallow at least one segment
        }
        let tail = &actual[prefix.len()..];
        if tail.iter().any(|segment| segment.is_empty()) {
            return false; // reject empties (`//`), like `{name}` does
        }
        return prefix
            .iter()
            .zip(actual.iter())
            .all(|(e, a)| segment_matches(e, a));
    }

    if expected.len() != actual.len() {
        return false;
    }
    expected
        .iter()
        .zip(actual.iter())
        .all(|(e, a)| segment_matches(e, a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_literal_template_matches_only_itself() {
        assert!(path_matches("/api/v1/users", "/api/v1/users"));
        assert!(!path_matches("/api/v1/users", "/api/v1/teams"));
        assert!(!path_matches("/api/v1/users", "/api/v1/users/42"));
    }

    #[test]
    fn a_placeholder_matches_exactly_one_non_empty_segment() {
        assert!(path_matches("/repos/{owner}/{repo}", "/repos/lumen/app"));
        assert!(!path_matches("/repos/{owner}/{repo}", "/repos/lumen"));
        assert!(!path_matches(
            "/repos/{owner}/{repo}",
            "/repos/lumen/app/issues"
        ));
        // An empty segment is not a resource id.
        assert!(!path_matches("/repos/{owner}/{repo}", "/repos//app"));
    }

    #[test]
    fn a_trailing_wildcard_swallows_a_slash_bearing_tail() {
        assert!(path_matches(
            "/repos/{owner}/{repo}/contents/{path...}",
            "/repos/lumen/app/contents/src/main.rs"
        ));
        assert!(path_matches(
            "/repos/{owner}/{repo}/contents/{path...}",
            "/repos/lumen/app/contents/README"
        ));
    }

    #[test]
    fn a_trailing_wildcard_requires_at_least_one_segment() {
        assert!(!path_matches(
            "/repos/{owner}/{repo}/contents/{path...}",
            "/repos/lumen/app/contents"
        ));
        assert!(!path_matches(
            "/repos/{owner}/{repo}/contents/{path...}",
            "/repos/lumen/app/contents/"
        ));
    }

    #[test]
    fn a_wildcard_tail_rejects_empty_segments() {
        assert!(!path_matches(
            "/repos/{owner}/{repo}/contents/{path...}",
            "/repos/lumen/app/contents/src//main.rs"
        ));
    }

    #[test]
    fn trailing_slashes_are_ignored_on_both_sides() {
        assert!(path_matches("/api/v1/users/", "/api/v1/users"));
        assert!(path_matches("/api/v1/users", "/api/v1/users/"));
        assert!(path_matches("/api/v1/users/", "/api/v1/users/"));
    }

    #[test]
    fn a_wildcard_outside_the_last_segment_is_rejected_at_load_time() {
        let bad = RestRoute {
            method: "GET".into(),
            path: "/repos/{path...}/contents".into(),
        };
        assert_eq!(
            bad.validate(),
            Err(RuleError::NonTrailingWildcard {
                path: "/repos/{path...}/contents".into()
            })
        );

        let good = RestRoute {
            method: "GET".into(),
            path: "/repos/{owner}/contents/{path...}".into(),
        };
        assert_eq!(good.validate(), Ok(()));
    }

    #[test]
    fn rules_round_trip_through_the_catalogs_json_shape() {
        let rest: MatchRule =
            serde_json::from_str(r#"{"kind":"rest","method":"GET","path":"/a/{b}"}"#).unwrap();
        assert_eq!(
            rest,
            MatchRule::Rest(RestRoute {
                method: "GET".into(),
                path: "/a/{b}".into()
            })
        );

        let graphql: MatchRule = serde_json::from_str(
            r#"{"kind":"graphql","operation_type":"mutation","field":"issueCreate"}"#,
        )
        .unwrap();
        assert_eq!(
            graphql,
            MatchRule::GraphQl(GraphQlOp {
                operation_type: "mutation".into(),
                field: "issueCreate".into()
            })
        );
    }
}
