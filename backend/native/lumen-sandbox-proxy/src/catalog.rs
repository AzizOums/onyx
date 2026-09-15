//! The external-app action catalog, generated from the Python providers.
//!
//! `scripts/export_catalog.py` writes `data/action_catalog.json` from
//! `lumen.external_apps.providers.registry`; this module embeds it at compile
//! time. The Python providers stay the single source of truth, and
//! `backend/tests/unit/lumen/sandbox_proxy/test_exported_catalog.py` fails when
//! the committed file drifts from them.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::matching::actions::{EndpointSpec, MatchRule};
use crate::matching::policy::EndpointPolicy;

const CATALOG_JSON: &str = include_str!("../data/action_catalog.json");

/// One action as exported, before the deployment's scope filter is applied.
#[derive(Debug, Clone, Deserialize)]
struct CatalogAction {
    id: String,
    normalised_name: String,
    description: String,
    default_policy: EndpointPolicy,
    /// True when the action needs a scope only a self-hosted deployment
    /// requests, which drops it from the cloud catalog.
    requires_self_hosted_scope: bool,
    matches: Vec<MatchRule>,
}

/// One built-in provider's entry in the catalog.
#[derive(Debug, Clone, Deserialize)]
pub struct CatalogApp {
    /// The `ExternalAppType` value, e.g. `"GITHUB"`.
    pub app_type: String,
    /// The provider's display name, as `_app_name` resolves it for a built-in.
    pub app_name: String,
    /// Match-ready regexes, authored by the provider and used as-is.
    pub upstream_url_regexes: Vec<String>,
    actions: Vec<CatalogAction>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawCatalog {
    apps: Vec<CatalogApp>,
}

/// Which OAuth grant the deployment connects built-in apps with.
///
/// On cloud, Lumen's own OAuth client is verified with the upstream provider
/// and narrows its scope, so the actions that client cannot cover drop out of
/// the catalog. Mirrors `registry.uses_cloud_scope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The deployment owns the OAuth credentials: the full catalog.
    SelfHosted,
    /// Lumen's cloud OAuth client: actions needing a self-hosted scope drop.
    Cloud,
}

impl CatalogApp {
    /// The action catalog for this app under `scope`.
    ///
    /// Every consumer funnels through here, so none of them can offer an action
    /// the grant in force will not authorize.
    pub fn endpoint_catalog(&self, scope: Scope) -> Vec<EndpointSpec> {
        self.actions
            .iter()
            .filter(|action| scope == Scope::SelfHosted || !action.requires_self_hosted_scope)
            .map(|action| EndpointSpec {
                id: action.id.clone(),
                normalised_name: action.normalised_name.clone(),
                description: action.description.clone(),
                matches: action.matches.clone(),
                default_policy: action.default_policy,
            })
            .collect()
    }
}

/// Every built-in provider, in the order the export wrote them.
pub fn apps() -> &'static [CatalogApp] {
    static CATALOG: OnceLock<Vec<CatalogApp>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            // A committed file that does not parse is a build-time mistake, not
            // a runtime condition: the gate has no catalog to decide with.
            let parsed: RawCatalog = serde_json::from_str(CATALOG_JSON)
                .expect("data/action_catalog.json is generated; regenerate it");
            parsed.apps
        })
        .as_slice()
}

/// The provider entry for an `ExternalAppType` value, or `None` for `CUSTOM`
/// and unregistered types.
pub fn app_for_type(app_type: &str) -> Option<&'static CatalogApp> {
    apps().iter().find(|app| app.app_type == app_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::actions::RestRoute;

    #[test]
    fn the_committed_catalog_parses_and_is_not_empty() {
        assert!(!apps().is_empty(), "the catalog has no providers");
        for app in apps() {
            assert!(!app.actions.is_empty(), "{} has no actions", app.app_type);
            assert!(
                !app.upstream_url_regexes.is_empty(),
                "{} claims no upstream URL",
                app.app_type
            );
        }
    }

    #[test]
    fn every_action_id_is_a_catalog_value_not_an_enum_repr() {
        // `str()` on the Python `str`-mixin enum renders "ClassName.MEMBER";
        // the database and the admin API key on the value. Exporting the wrong
        // one would make every stored policy override silently miss.
        for app in apps() {
            for action in &app.actions {
                assert!(
                    !action.id.contains("Action."),
                    "{} exported an enum repr as an id: {}",
                    app.app_type,
                    action.id
                );
                assert!(
                    action.id.contains('.'),
                    "{} has an unnamespaced action id: {}",
                    app.app_type,
                    action.id
                );
            }
        }
    }

    #[test]
    fn action_ids_are_unique_within_an_app() {
        // Policies key on the id, so a duplicate would make one action's
        // override govern another's.
        for app in apps() {
            let mut seen = std::collections::HashSet::new();
            for action in &app.actions {
                assert!(
                    seen.insert(action.id.as_str()),
                    "{} has a duplicate action id: {}",
                    app.app_type,
                    action.id
                );
            }
        }
    }

    #[test]
    fn every_upstream_regex_compiles_in_rust() {
        // Python's `re` accepts constructs this crate does not (backreferences,
        // lookaround). A provider regex that fails here would make its app
        // unresolvable, and the request would forward ungated — so it is a
        // build-time failure, not a warning at runtime.
        for app in apps() {
            for pattern in &app.upstream_url_regexes {
                regex::Regex::new(pattern).unwrap_or_else(|error| {
                    panic!(
                        "{} regex {pattern:?} does not compile: {error}",
                        app.app_type
                    )
                });
            }
        }
    }

    #[test]
    fn every_rest_route_passes_the_catalog_load_time_validation() {
        for app in apps() {
            for action in &app.actions {
                for rule in &action.matches {
                    if let MatchRule::Rest(route) = rule {
                        route.validate().unwrap_or_else(|error| {
                            panic!("{} action {}: {error}", app.app_type, action.id)
                        });
                    }
                }
            }
        }
    }

    #[test]
    fn a_graphql_rule_names_a_real_operation_type() {
        for app in apps() {
            for action in &app.actions {
                for rule in &action.matches {
                    if let MatchRule::GraphQl(op) = rule {
                        assert!(
                            matches!(
                                op.operation_type.as_str(),
                                "query" | "mutation" | "subscription"
                            ),
                            "{} action {} names operation type {:?}",
                            app.app_type,
                            action.id,
                            op.operation_type
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_cloud_scope_never_offers_more_than_the_self_hosted_one() {
        for app in apps() {
            let cloud = app.endpoint_catalog(Scope::Cloud);
            let self_hosted = app.endpoint_catalog(Scope::SelfHosted);
            assert!(cloud.len() <= self_hosted.len());
            let self_hosted_ids: std::collections::HashSet<&str> =
                self_hosted.iter().map(|e| e.id.as_str()).collect();
            for endpoint in &cloud {
                assert!(self_hosted_ids.contains(endpoint.id.as_str()));
            }
        }
    }

    #[test]
    fn a_known_github_action_is_reachable_through_the_catalog() {
        // An end-to-end sanity check on the whole load path, against an action
        // whose shape is stable.
        let github = app_for_type("GITHUB").expect("GitHub is a built-in provider");
        let catalog = github.endpoint_catalog(Scope::SelfHosted);
        let read_user = catalog
            .iter()
            .find(|e| e.id == "github.user.read")
            .expect("github.user.read is in the catalog");
        assert!(read_user.matches.contains(&MatchRule::Rest(RestRoute {
            method: "GET".into(),
            path: "/user".into(),
        })));
    }

    #[test]
    fn an_unregistered_app_type_has_no_catalog() {
        assert!(app_for_type("CUSTOM").is_none());
        assert!(app_for_type("NOT_AN_APP").is_none());
    }
}
