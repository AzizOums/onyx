//! Recognising which catalog action an outbound sandbox request invokes, and
//! what policy governs it.
//!
//! Port of `backend/lumen/external_apps/matching/` plus the pieces of
//! `providers/actions.py`, `providers/registry.py` and `url_glob.py` the
//! runtime gate depends on. Everything here is pure: no database, no
//! credentials, no network. The catalog it matches against is generated from
//! the Python providers — see `crate::catalog`.

pub mod actions;
pub mod engine;
pub mod graphql;
pub mod policy;
pub mod url_glob;

pub use actions::{path_matches, EndpointSpec, GraphQlOp, MatchRule, RestRoute};
pub use engine::{
    actions_requiring_approval, apply_credential_gate, effective_policy, recognize_actions,
    AllMatchedActions, GatedTarget, MatchContext, MatchedAction, ProxiedRequest,
    WHOLE_DOMAIN_ACTION_TYPE,
};
pub use policy::{EndpointPolicy, GatedAppKind};
pub use url_glob::{UrlGlob, UrlGlobError};
