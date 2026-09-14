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

/// Environment variable naming the directory of extra CA roots to trust.
///
/// Same contract the distroless model server follows: the Helm chart mounts the
/// bundle and names the directory here, because a distroless image has no shell
/// to run `update-ca-certificates`. Any file name under the directory works, and
/// the roots are additive to the public ones.
pub const CUSTOM_CA_CERTS_DIR_ENV: &str = "LUMEN_CUSTOM_CA_CERTS_DIR";

/// Load every PEM certificate under `LUMEN_CUSTOM_CA_CERTS_DIR`.
///
/// A no-op unless the variable is set, so default deployments are unaffected. A
/// file that fails to parse is logged and skipped rather than failing startup:
/// one bad file in the mount should not take the server down.
fn custom_root_certificates() -> Vec<reqwest::Certificate> {
    let Some(dir) = std::env::var(CUSTOM_CA_CERTS_DIR_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Vec::new();
    };

    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) => {
            tracing::error!(dir = %dir, error = %err, "Cannot read the custom CA directory");
            return Vec::new();
        }
    };

    let mut certificates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match std::fs::read(&path) {
            // One file may hold a chain, so parse it as a bundle.
            Ok(bytes) => match reqwest::Certificate::from_pem_bundle(&bytes) {
                Ok(parsed) => certificates.extend(parsed),
                Err(err) => tracing::warn!(
                    path = %path.display(),
                    error = %err,
                    "Skipping an unreadable custom CA file"
                ),
            },
            Err(err) => tracing::warn!(
                path = %path.display(),
                error = %err,
                "Cannot read a custom CA file"
            ),
        }
    }

    if !certificates.is_empty() {
        tracing::info!(
            dir = %dir,
            count = certificates.len(),
            "Trusting custom CA roots in addition to the public ones"
        );
    }
    certificates
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
        let mut builder = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(CONNECT_TIMEOUT);

        for certificate in custom_root_certificates() {
            builder = builder.add_root_certificate(certificate);
        }

        Ok(Self {
            http: builder.build()?,
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
    use std::sync::Mutex;

    // These tests mutate a process-wide variable, so they take a lock.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

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

    /// A self-signed certificate, only ever parsed — never trusted by a test
    /// that reaches the network.
    const TEST_CA_PEM: &str = include_str!("../tests/fixtures/test-ca.pem");

    #[test]
    fn no_custom_ca_directory_means_no_extra_roots() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::remove_var(CUSTOM_CA_CERTS_DIR_ENV);
        assert!(custom_root_certificates().is_empty());

        // An empty or whitespace-only value counts as unset.
        for value in ["", "   "] {
            std::env::set_var(CUSTOM_CA_CERTS_DIR_ENV, value);
            assert!(custom_root_certificates().is_empty(), "value {value:?}");
        }
        std::env::remove_var(CUSTOM_CA_CERTS_DIR_ENV);
    }

    #[test]
    fn every_pem_under_the_directory_is_loaded_and_bad_files_are_skipped() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());

        let dir = std::env::temp_dir().join(format!("lumen-ca-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the temp directory is created");
        std::fs::write(dir.join("root.pem"), TEST_CA_PEM).expect("the PEM is written");
        std::fs::write(dir.join("not-a-cert.txt"), b"garbage").expect("the junk is written");
        // A subdirectory must not be read as a file.
        std::fs::create_dir_all(dir.join("nested")).expect("the subdirectory is created");

        std::env::set_var(CUSTOM_CA_CERTS_DIR_ENV, &dir);
        let certificates = custom_root_certificates();
        std::env::remove_var(CUSTOM_CA_CERTS_DIR_ENV);
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(certificates.len(), 1);
    }

    #[test]
    fn a_missing_custom_ca_directory_does_not_fail_startup() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var(CUSTOM_CA_CERTS_DIR_ENV, "/definitely/not/here");
        let certificates = custom_root_certificates();
        std::env::remove_var(CUSTOM_CA_CERTS_DIR_ENV);
        assert!(certificates.is_empty());
    }

    #[test]
    fn urls_join_with_exactly_one_slash() {
        let client = ApiClient::with_base_url("http://api/api/", Duration::from_secs(1)).unwrap();
        assert_eq!(client.url("/search"), "http://api/api/search");
        assert_eq!(client.url("search"), "http://api/api/search");
    }
}
