//! HTTP client for the Lumen API.
//!
//! The MCP server owns no data. Every resource and tool call forwards to the API
//! server with the caller's bearer token, so this module is the only place that
//! talks to it.

use std::time::Duration;

use reqwest::{Response, StatusCode};
use serde::Serialize;

use crate::config::{Config, CONNECT_TIMEOUT};

#[derive(Debug, thiserror::Error)]
pub enum UpstreamError {
    /// The request never produced a response: DNS, connect, TLS or timeout.
    #[error("request to the Lumen API failed: {0}")]
    Transport(#[from] reqwest::Error),
    /// The API answered, but not with success. `detail` is already
    /// caller-readable.
    #[error("{detail}")]
    Status { status: StatusCode, detail: String },
    /// The API answered with success but a body we could not read.
    #[error("unexpected response from the Lumen API: {0}")]
    Body(String),
}

impl UpstreamError {
    pub fn status(&self) -> Option<StatusCode> {
        match self {
            Self::Status { status, .. } => Some(*status),
            _ => None,
        }
    }
}

/// Shared client for the Lumen API, cloned freely: `reqwest::Client` is an
/// `Arc` internally and pools connections across calls.
#[derive(Debug, Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    base_url: String,
}

impl ApiClient {
    pub fn new(config: &Config) -> Result<Self, reqwest::Error> {
        Self::with_base_url(&config.api_base_url, config.api_request_timeout)
    }

    /// Split out so tests can point the client at a local mock API.
    pub fn with_base_url(base_url: &str, timeout: Duration) -> Result<Self, reqwest::Error> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(timeout)
                .connect_timeout(CONNECT_TIMEOUT)
                .build()?,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    /// GET a path as the token's owner. Returns the raw response so callers can
    /// tell a rejected token from a transport failure.
    pub async fn get(&self, path: &str, token: &str) -> Result<Response, reqwest::Error> {
        self.http
            .get(self.url(path))
            .bearer_auth(token)
            .send()
            .await
    }

    /// POST a JSON body as the token's owner.
    pub async fn post_json<T: Serialize>(
        &self,
        path: &str,
        token: &str,
        body: &T,
    ) -> Result<Response, reqwest::Error> {
        self.http
            .post(self.url(path))
            .bearer_auth(token)
            .json(body)
            .send()
            .await
    }

    /// GET and deserialize, turning a non-success status into `Status`.
    pub async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        token: &str,
    ) -> Result<T, UpstreamError> {
        let response = self.get(path, token).await?;
        deserialize(response).await
    }

    /// POST and deserialize, turning a non-success status into `Status`.
    pub async fn post_json_for<B: Serialize, T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        token: &str,
        body: &B,
    ) -> Result<T, UpstreamError> {
        let response = self.post_json(path, token, body).await?;
        deserialize(response).await
    }
}

async fn deserialize<T: serde::de::DeserializeOwned>(
    response: Response,
) -> Result<T, UpstreamError> {
    let status = response.status();
    let body = response.bytes().await?;

    if !status.is_success() {
        return Err(UpstreamError::Status {
            status,
            detail: error_detail(status, &body),
        });
    }

    serde_json::from_slice(&body).map_err(|err| UpstreamError::Body(err.to_string()))
}

/// Port of `_extract_error_detail`.
///
/// The backend returns `LumenError` as `{"error_code": ..., "detail": ...}`.
/// Anything else degrades to the same generic sentence the Python server emits,
/// because MCP clients surface this string to the user.
pub fn error_detail(status: StatusCode, body: &[u8]) -> String {
    if let Ok(serde_json::Value::Object(map)) = serde_json::from_slice::<serde_json::Value>(body) {
        if let Some(detail) = map.get("detail") {
            // Python does `str(detail)`, which keeps a non-string detail readable.
            let rendered = match detail {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            if !rendered.is_empty() {
                return rendered;
            }
        }
    }
    format!("Request failed with status {}", status.as_u16())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_detail_reads_the_lumen_error_envelope() {
        let body = br#"{"error_code":"NOT_FOUND","detail":"Agent not found"}"#;
        assert_eq!(error_detail(StatusCode::NOT_FOUND, body), "Agent not found");
    }

    #[test]
    fn error_detail_falls_back_on_a_non_json_body() {
        assert_eq!(
            error_detail(StatusCode::BAD_GATEWAY, b"<html>oops</html>"),
            "Request failed with status 502"
        );
    }

    #[test]
    fn error_detail_falls_back_when_detail_is_absent_or_empty() {
        assert_eq!(
            error_detail(StatusCode::IM_A_TEAPOT, br#"{"error_code":"X"}"#),
            "Request failed with status 418"
        );
        assert_eq!(
            error_detail(StatusCode::IM_A_TEAPOT, br#"{"detail":""}"#),
            "Request failed with status 418"
        );
    }

    #[test]
    fn error_detail_renders_a_structured_detail() {
        let body = br#"{"detail":{"field":"query"}}"#;
        assert_eq!(
            error_detail(StatusCode::UNPROCESSABLE_ENTITY, body),
            r#"{"field":"query"}"#
        );
    }

    #[test]
    fn urls_join_with_exactly_one_slash() {
        let client = ApiClient::with_base_url("http://api/api/", Duration::from_secs(1)).unwrap();
        assert_eq!(client.url("/search"), "http://api/api/search");
        assert_eq!(client.url("search"), "http://api/api/search");
    }
}
