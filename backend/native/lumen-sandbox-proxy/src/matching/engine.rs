//! Composes recognition and policy resolution into a single verdict for an
//! outbound request.
//!
//! Port of `backend/lumen/external_apps/matching/engine.py` and the
//! `MatchContext` half of `matching/request.py`.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use super::actions::{path_matches, EndpointSpec, MatchRule};
use super::graphql::parse_invocations;
use super::policy::{EndpointPolicy, GatedAppKind};

/// `action_type` for a domain-matched request that hit no catalog action.
pub const WHOLE_DOMAIN_ACTION_TYPE: &str = "unspecified";

/// The normalised form of an outbound sandbox request, transport-agnostic.
///
/// The host is already consumed by the proxy's app match, so only the parts
/// that distinguish *actions within an app* are carried here.
#[derive(Debug, Clone, Default)]
pub struct ProxiedRequest {
    /// HTTP verb; compared case-insensitively.
    pub method: String,
    /// URL path, tested against a `RestRoute` path template.
    pub path: String,
    /// Raw body; parsed lazily for GraphQL matching.
    pub body: Option<Vec<u8>>,
}

impl ProxiedRequest {
    /// Normalise an intercepted request into the form the matcher consumes.
    ///
    /// `raw_path` is the path as the interception layer reports it, which
    /// carries the query string; catalog path matchers test the path only, so
    /// it is dropped here. Mirrors `ExternalAppRequestEvaluator.evaluate`.
    pub fn from_http(method: &str, raw_path: &str, body: Option<Vec<u8>>) -> Self {
        Self {
            method: method.to_string(),
            path: raw_path.split('?').next().unwrap_or("").to_string(),
            body,
        }
    }
}

/// A request paired with the derived views matchers consult.
///
/// Interpreting a request (parsing a GraphQL body) happens here and is
/// memoised: it runs at most once per request, and only if some rule asks for
/// it. A pure REST app never triggers GraphQL parsing.
pub struct MatchContext<'a> {
    request: &'a ProxiedRequest,
    graphql_invocations: std::cell::OnceCell<Vec<(String, String)>>,
}

impl<'a> MatchContext<'a> {
    pub fn new(request: &'a ProxiedRequest) -> Self {
        Self {
            request,
            graphql_invocations: std::cell::OnceCell::new(),
        }
    }

    pub fn request(&self) -> &ProxiedRequest {
        self.request
    }

    /// `(operation_type, root_field)` pairs the request's body invokes.
    pub fn graphql_invocations(&self) -> &[(String, String)] {
        self.graphql_invocations
            .get_or_init(|| parse_invocations(self.request.body.as_deref()))
    }
}

/// Whether one rule fires for a request.
pub fn rule_matches(rule: &MatchRule, context: &MatchContext<'_>) -> bool {
    match rule {
        MatchRule::Rest(route) => {
            route.method.eq_ignore_ascii_case(&context.request().method)
                && path_matches(&route.path, &context.request().path)
        }
        MatchRule::GraphQl(op) => {
            context
                .graphql_invocations()
                .iter()
                .any(|(operation_type, field)| {
                    operation_type == &op.operation_type && field == &op.field
                })
        }
    }
}

/// One catalog action a request invoked, with the display strings the frontend
/// renders. Carried verbatim from matcher through the database to the API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchedAction {
    pub action_type: String,
    pub display_name: String,
    pub description: String,
    pub policy: EndpointPolicy,
}

/// The connected app or server a gated request is attributed to.
///
/// `id` indexes the table named by `kind`. Lookups key off `(kind, id)`, never
/// `app_name`: the latter is not unique across instances.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatedTarget {
    pub kind: GatedAppKind,
    pub id: i32,
    pub app_name: String,
}

impl GatedTarget {
    /// The `(kind, id)` pair, so a live match compares directly against a
    /// persisted grant.
    pub fn key(&self) -> (GatedAppKind, i32) {
        (self.kind, self.id)
    }
}

/// Every catalog action the request matched within the resolved app.
///
/// `actions` is sorted strictest-policy-first and is never empty, so
/// [`AllMatchedActions::governing_action`] always exists and drives the gate's
/// verdict. A batched GraphQL POST is the canonical multi-action case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllMatchedActions {
    actions: Vec<MatchedAction>,
    pub target: GatedTarget,
    #[serde(default)]
    pub payload: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MatchError {
    #[error("AllMatchedActions.actions must be non-empty")]
    NoActions,
}

impl AllMatchedActions {
    /// Sorts strictest-first and rejects empty, so `actions[0]` always drives
    /// the verdict however a producer built the list.
    pub fn new(mut actions: Vec<MatchedAction>, target: GatedTarget) -> Result<Self, MatchError> {
        if actions.is_empty() {
            return Err(MatchError::NoActions);
        }
        // A stable sort, like Python's: equal-severity actions keep catalog
        // order, which is what makes the verdict reproducible.
        actions.sort_by(|a, b| b.policy.severity().cmp(&a.policy.severity()));
        Ok(Self {
            actions,
            target,
            payload: serde_json::Map::new(),
        })
    }

    pub fn actions(&self) -> &[MatchedAction] {
        &self.actions
    }

    /// The action whose policy drove the verdict (head of the sorted list).
    pub fn governing_action(&self) -> &MatchedAction {
        &self.actions[0]
    }

    pub fn app_name(&self) -> &str {
        &self.target.app_name
    }

    pub fn with_payload(mut self, payload: serde_json::Map<String, serde_json::Value>) -> Self {
        self.payload = payload;
        self
    }

    /// Keep only the actions `keep` accepts, or `None` when none survive.
    fn retaining(&self, keep: impl Fn(&MatchedAction) -> bool) -> Option<Self> {
        let kept: Vec<MatchedAction> = self.actions.iter().filter(|a| keep(a)).cloned().collect();
        if kept.is_empty() {
            return None;
        }
        Some(Self {
            actions: kept,
            target: self.target.clone(),
            payload: self.payload.clone(),
        })
    }
}

/// The action types whose policy requires a user approval.
///
/// Returned sorted and de-duplicated, so a caller comparing a live request
/// against a persisted grant compares the same shape either way.
pub fn actions_requiring_approval<'a>(
    actions: impl IntoIterator<Item = &'a MatchedAction>,
) -> Vec<String> {
    let unique: BTreeSet<&str> = actions
        .into_iter()
        .filter(|action| action.policy == EndpointPolicy::Ask)
        .map(|action| action.action_type.as_str())
        .collect();
    unique.into_iter().map(str::to_string).collect()
}

/// Which catalog action(s) `request` invokes within an app — pure recognition,
/// no credential knowledge.
///
/// `stored` holds the admin's per-action overrides; an action with no entry
/// keeps its catalog default. Returns `None` when no catalog action matches;
/// the credential gate and the whole-domain fallback are the caller's job (see
/// [`apply_credential_gate`]).
pub fn recognize_actions(
    catalog: &[EndpointSpec],
    stored: &HashMap<String, EndpointPolicy>,
    target: &GatedTarget,
    request: &ProxiedRequest,
) -> Option<AllMatchedActions> {
    let context = MatchContext::new(request);
    let matched: Vec<MatchedAction> = catalog
        .iter()
        .filter(|endpoint| {
            endpoint
                .matches
                .iter()
                .any(|rule| rule_matches(rule, &context))
        })
        .map(|endpoint| MatchedAction {
            action_type: endpoint.id.clone(),
            display_name: endpoint.normalised_name.clone(),
            description: endpoint.description.clone(),
            policy: effective_policy(endpoint, stored),
        })
        .collect();

    if matched.is_empty() {
        return None;
    }
    // Non-empty by construction.
    AllMatchedActions::new(matched, target.clone()).ok()
}

/// The policy in force for an endpoint: the admin's stored override, else the
/// catalog's curated default.
///
/// Shared by the runtime gate and the admin view so the two never diverge —
/// notably during catalog drift, when a newly-shipped endpoint has no stored
/// row yet.
pub fn effective_policy(
    endpoint: &EndpointSpec,
    stored: &HashMap<String, EndpointPolicy>,
) -> EndpointPolicy {
    stored
        .get(&endpoint.id)
        .copied()
        .unwrap_or(endpoint.default_policy)
}

/// Apply the credential gate to a pure [`recognize_actions`] result, given
/// whether the app `is_available` (active and injectable — the caller resolves
/// that). Pure: no database or credential access.
///
/// - available + a catalog action matched → those actions, unchanged.
/// - available + nothing matched → gate the whole domain under a default `ASK`.
/// - not available → keep only a recorded `DENY` (an explicit block fires with
///   or without a credential), else `None` (forward the request bare, no
///   prompt).
pub fn apply_credential_gate(
    target: &GatedTarget,
    request: &ProxiedRequest,
    matched_actions: Option<AllMatchedActions>,
    is_available: bool,
) -> Option<AllMatchedActions> {
    if !is_available {
        let matched = matched_actions?;
        return matched.retaining(|action| action.policy == EndpointPolicy::Deny);
    }
    if let Some(matched) = matched_actions {
        return Some(matched);
    }
    AllMatchedActions::new(
        vec![MatchedAction {
            action_type: WHOLE_DOMAIN_ACTION_TYPE.to_string(),
            display_name: "Perform action".to_string(),
            description: format!("{} {}", request.method, request.path),
            policy: EndpointPolicy::Ask,
        }],
        target.clone(),
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::actions::{GraphQlOp, RestRoute};

    fn target() -> GatedTarget {
        GatedTarget {
            kind: GatedAppKind::ExternalApp,
            id: 7,
            app_name: "GitHub".into(),
        }
    }

    fn endpoint(id: &str, policy: EndpointPolicy, matches: Vec<MatchRule>) -> EndpointSpec {
        EndpointSpec {
            id: id.into(),
            normalised_name: id.into(),
            description: format!("does {id}"),
            matches,
            default_policy: policy,
        }
    }

    fn rest(method: &str, path: &str) -> MatchRule {
        MatchRule::Rest(RestRoute {
            method: method.into(),
            path: path.into(),
        })
    }

    fn get(path: &str) -> ProxiedRequest {
        ProxiedRequest {
            method: "GET".into(),
            path: path.into(),
            body: None,
        }
    }

    #[test]
    fn a_request_matching_no_catalog_action_recognizes_nothing() {
        let catalog = vec![endpoint(
            "repo.read",
            EndpointPolicy::Always,
            vec![rest("GET", "/repos/{owner}/{repo}")],
        )];
        assert!(recognize_actions(&catalog, &HashMap::new(), &target(), &get("/teams")).is_none());
    }

    #[test]
    fn a_stored_override_beats_the_catalog_default() {
        let catalog = vec![endpoint(
            "repo.read",
            EndpointPolicy::Always,
            vec![rest("GET", "/repos/{owner}/{repo}")],
        )];
        let stored = HashMap::from([("repo.read".to_string(), EndpointPolicy::Deny)]);
        let matched =
            recognize_actions(&catalog, &stored, &target(), &get("/repos/lumen/app")).unwrap();
        assert_eq!(matched.governing_action().policy, EndpointPolicy::Deny);
    }

    #[test]
    fn a_method_is_compared_case_insensitively() {
        let catalog = vec![endpoint(
            "repo.read",
            EndpointPolicy::Ask,
            vec![rest("get", "/repos/{owner}/{repo}")],
        )];
        let request = ProxiedRequest {
            method: "GeT".into(),
            path: "/repos/lumen/app".into(),
            body: None,
        };
        assert!(recognize_actions(&catalog, &HashMap::new(), &target(), &request).is_some());
    }

    #[test]
    fn the_strictest_matched_policy_governs() {
        let catalog = vec![
            endpoint("a.always", EndpointPolicy::Always, vec![rest("GET", "/x")]),
            endpoint("b.deny", EndpointPolicy::Deny, vec![rest("GET", "/x")]),
            endpoint("c.ask", EndpointPolicy::Ask, vec![rest("GET", "/x")]),
        ];
        let matched = recognize_actions(&catalog, &HashMap::new(), &target(), &get("/x")).unwrap();
        let policies: Vec<_> = matched.actions().iter().map(|a| a.policy).collect();
        assert_eq!(
            policies,
            vec![
                EndpointPolicy::Deny,
                EndpointPolicy::Ask,
                EndpointPolicy::Always
            ]
        );
        assert_eq!(matched.governing_action().action_type, "b.deny");
    }

    #[test]
    fn equal_severity_actions_keep_catalog_order() {
        // A stable sort is what makes two runs of the same request agree, and
        // what makes the Rust verdict reproduce the Python one.
        let catalog = vec![
            endpoint("first", EndpointPolicy::Ask, vec![rest("GET", "/x")]),
            endpoint("second", EndpointPolicy::Ask, vec![rest("GET", "/x")]),
            endpoint("third", EndpointPolicy::Ask, vec![rest("GET", "/x")]),
        ];
        let matched = recognize_actions(&catalog, &HashMap::new(), &target(), &get("/x")).unwrap();
        let ids: Vec<_> = matched
            .actions()
            .iter()
            .map(|a| a.action_type.as_str())
            .collect();
        assert_eq!(ids, vec!["first", "second", "third"]);
    }

    #[test]
    fn an_unavailable_app_keeps_only_an_explicit_deny() {
        let catalog = vec![
            endpoint("a.ask", EndpointPolicy::Ask, vec![rest("GET", "/x")]),
            endpoint("b.deny", EndpointPolicy::Deny, vec![rest("GET", "/x")]),
        ];
        let matched = recognize_actions(&catalog, &HashMap::new(), &target(), &get("/x"));
        let gated = apply_credential_gate(&target(), &get("/x"), matched, false).unwrap();
        let ids: Vec<_> = gated
            .actions()
            .iter()
            .map(|a| a.action_type.as_str())
            .collect();
        assert_eq!(ids, vec!["b.deny"]);
    }

    #[test]
    fn an_unavailable_app_with_no_deny_forwards_bare() {
        let catalog = vec![endpoint(
            "a.ask",
            EndpointPolicy::Ask,
            vec![rest("GET", "/x")],
        )];
        let matched = recognize_actions(&catalog, &HashMap::new(), &target(), &get("/x"));
        assert!(apply_credential_gate(&target(), &get("/x"), matched, false).is_none());
    }

    #[test]
    fn an_available_app_gates_its_whole_domain_when_nothing_matched() {
        let gated = apply_credential_gate(&target(), &get("/unknown"), None, true).unwrap();
        let action = gated.governing_action();
        assert_eq!(action.action_type, WHOLE_DOMAIN_ACTION_TYPE);
        assert_eq!(action.policy, EndpointPolicy::Ask);
        assert_eq!(action.description, "GET /unknown");
    }

    #[test]
    fn an_unavailable_app_that_matched_nothing_is_not_gated() {
        assert!(apply_credential_gate(&target(), &get("/unknown"), None, false).is_none());
    }

    #[test]
    fn only_ask_actions_require_an_approval() {
        let actions = vec![
            MatchedAction {
                action_type: "b.ask".into(),
                display_name: "b".into(),
                description: String::new(),
                policy: EndpointPolicy::Ask,
            },
            MatchedAction {
                action_type: "a.ask".into(),
                display_name: "a".into(),
                description: String::new(),
                policy: EndpointPolicy::Ask,
            },
            MatchedAction {
                action_type: "c.deny".into(),
                display_name: "c".into(),
                description: String::new(),
                policy: EndpointPolicy::Deny,
            },
            MatchedAction {
                action_type: "d.always".into(),
                display_name: "d".into(),
                description: String::new(),
                policy: EndpointPolicy::Always,
            },
            MatchedAction {
                action_type: "a.ask".into(),
                display_name: "a again".into(),
                description: String::new(),
                policy: EndpointPolicy::Ask,
            },
        ];
        // Sorted and de-duplicated, so a live request and a persisted grant
        // compare as the same list.
        assert_eq!(actions_requiring_approval(&actions), vec!["a.ask", "b.ask"]);
    }

    #[test]
    fn a_graphql_body_is_parsed_only_when_a_rule_asks_for_it() {
        let catalog = vec![endpoint(
            "linear.issue.create",
            EndpointPolicy::Ask,
            vec![MatchRule::GraphQl(GraphQlOp {
                operation_type: "mutation".into(),
                field: "issueCreate".into(),
            })],
        )];
        let request = ProxiedRequest {
            method: "POST".into(),
            path: "/graphql".into(),
            body: Some(br#"{"query":"mutation { issueCreate { id } }"}"#.to_vec()),
        };
        let matched = recognize_actions(&catalog, &HashMap::new(), &target(), &request).unwrap();
        assert_eq!(
            matched.governing_action().action_type,
            "linear.issue.create"
        );

        // A query of the same field is a different action, and matches nothing.
        let read = ProxiedRequest {
            body: Some(br#"{"query":"query { issueCreate { id } }"#.to_vec()),
            ..request
        };
        assert!(recognize_actions(&catalog, &HashMap::new(), &target(), &read).is_none());
    }

    #[test]
    fn a_target_compares_by_kind_and_id_not_by_name() {
        let external = GatedTarget {
            kind: GatedAppKind::ExternalApp,
            id: 1,
            app_name: "Jira".into(),
        };
        let mcp = GatedTarget {
            kind: GatedAppKind::McpServer,
            id: 1,
            app_name: "Jira".into(),
        };
        assert_ne!(external.key(), mcp.key());
    }
}

#[cfg(test)]
mod normalization_tests {
    use super::*;

    #[test]
    fn a_query_string_is_dropped_before_matching() {
        // The interception layer reports the path with its query string;
        // catalog path matchers test the path only, so a route would never
        // match a real request if this were kept.
        let request = ProxiedRequest::from_http("GET", "/repos/lumen/app?per_page=100", None);
        assert_eq!(request.path, "/repos/lumen/app");
        assert_eq!(request.method, "GET");
    }

    #[test]
    fn only_the_first_question_mark_splits_the_path() {
        let request = ProxiedRequest::from_http("GET", "/a?b=1?c=2", None);
        assert_eq!(request.path, "/a");
    }

    #[test]
    fn a_path_without_a_query_string_is_unchanged() {
        let request = ProxiedRequest::from_http("POST", "/graphql", Some(b"{}".to_vec()));
        assert_eq!(request.path, "/graphql");
        assert_eq!(request.body.as_deref(), Some(b"{}".as_slice()));
    }
}
