//! Bearer-token authentication.
//!
//! Port of `LumenTokenVerifier` in `backend/lumen/mcp_server/auth.py`. The MCP
//! server holds no user store: it asks the Lumen API whether a token is good by
//! calling `/me` with it, and keeps nothing but the token itself.

use crate::metrics::{AuthResult, METRICS};
use crate::upstream::ApiClient;

/// A token the API server accepted.
///
/// Mirrors the minimal `AccessToken` the Python verifier returns: the client id
/// and scope are fixed, and no claims are read out of the token.
#[derive(Debug, Clone)]
pub struct AccessToken {
    pub token: String,
}

impl AccessToken {
    pub const CLIENT_ID: &'static str = "mcp";
    pub const SCOPES: [&'static str; 1] = ["mcp:use"];
}

/// Verify a token against the Lumen API.
///
/// Returns `None` both when the API rejects the token and when the API cannot be
/// reached — the caller cannot tell the difference, exactly as in Python, but
/// the two cases are recorded under different metric labels and log levels.
pub async fn verify_token(client: &ApiClient, token: &str) -> Option<AccessToken> {
    let response = match client.get("/me", token).await {
        Ok(response) => response,
        Err(err) => {
            METRICS.record_auth(AuthResult::Error);
            tracing::error!(
                error = %err,
                "MCP server failed to reach API /me for authentication"
            );
            return None;
        }
    };

    if response.status() != reqwest::StatusCode::OK {
        METRICS.record_auth(AuthResult::Rejected);
        tracing::warn!(
            status = response.status().as_u16(),
            "API server rejected MCP auth token"
        );
        return None;
    }

    METRICS.record_auth(AuthResult::Success);
    Some(AccessToken {
        token: token.to_string(),
    })
}

/// Pull the bearer token out of an `Authorization` header value.
///
/// The scheme match is case-insensitive, as RFC 7235 requires.
pub fn bearer_token(header: &str) -> Option<&str> {
    let (scheme, token) = header.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    (!token.is_empty()).then_some(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_scheme_is_case_insensitive() {
        assert_eq!(bearer_token("Bearer abc"), Some("abc"));
        assert_eq!(bearer_token("bearer abc"), Some("abc"));
        assert_eq!(bearer_token("BEARER abc"), Some("abc"));
    }

    #[test]
    fn other_schemes_and_empty_tokens_are_rejected() {
        assert_eq!(bearer_token("Basic abc"), None);
        assert_eq!(bearer_token("abc"), None);
        assert_eq!(bearer_token("Bearer "), None);
        assert_eq!(bearer_token("Bearer    "), None);
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(bearer_token("Bearer  abc  "), Some("abc"));
    }

    #[test]
    fn access_token_constants_match_the_python_verifier() {
        assert_eq!(AccessToken::CLIENT_ID, "mcp");
        assert_eq!(AccessToken::SCOPES, ["mcp:use"]);
    }
}
