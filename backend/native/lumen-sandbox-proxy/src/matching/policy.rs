//! Policies and their strictness ordering.
//!
//! Port of the `EndpointPolicy` / `POLICY_SEVERITY` half of
//! `backend/lumen/db/enums.py`, and of `GatedAppKind`.

use serde::{Deserialize, Serialize};

/// What the egress layer does with a request once it matched an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EndpointPolicy {
    /// Auto-approve: the call proceeds without prompting.
    #[serde(rename = "ALWAYS")]
    Always,
    /// Require approval: the user accepts or denies in-session.
    #[serde(rename = "ASK")]
    Ask,
    /// Block the call outright.
    #[serde(rename = "DENY")]
    Deny,
}

impl EndpointPolicy {
    /// Strictness: higher is stricter. When one request matches several
    /// actions, the strictest governs.
    pub fn severity(self) -> u8 {
        match self {
            EndpointPolicy::Always => 0,
            EndpointPolicy::Ask => 1,
            EndpointPolicy::Deny => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            EndpointPolicy::Always => "ALWAYS",
            EndpointPolicy::Ask => "ASK",
            EndpointPolicy::Deny => "DENY",
        }
    }
}

/// Which catalog a gated action belongs to — the target id indexes that table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GatedAppKind {
    #[serde(rename = "EXTERNAL_APP")]
    ExternalApp,
    #[serde(rename = "MCP_SERVER")]
    McpServer,
}

impl GatedAppKind {
    pub fn as_str(self) -> &'static str {
        match self {
            GatedAppKind::ExternalApp => "EXTERNAL_APP",
            GatedAppKind::McpServer => "MCP_SERVER",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_is_stricter_than_ask_is_stricter_than_always() {
        assert!(EndpointPolicy::Deny.severity() > EndpointPolicy::Ask.severity());
        assert!(EndpointPolicy::Ask.severity() > EndpointPolicy::Always.severity());
    }

    #[test]
    fn policies_serialize_as_the_strings_the_database_stores() {
        let json = serde_json::to_string(&EndpointPolicy::Ask).unwrap();
        assert_eq!(json, "\"ASK\"");
        let parsed: EndpointPolicy = serde_json::from_str("\"DENY\"").unwrap();
        assert_eq!(parsed, EndpointPolicy::Deny);
    }
}
