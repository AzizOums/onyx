//! A stand-in for the Lumen API.
//!
//! Serves the seven endpoints the MCP server calls, with per-endpoint overrides
//! so a test can force a status, a body, or a hang. Records every request so
//! tests can assert on what the server actually sent upstream.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde_json::{json, Value};

/// What the mock should answer for one endpoint.
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: StatusCode,
    pub body: String,
}

impl Reply {
    pub fn ok(body: Value) -> Self {
        Self {
            status: StatusCode::OK,
            body: body.to_string(),
        }
    }

    pub fn raw(status: StatusCode, body: &str) -> Self {
        Self {
            status,
            body: body.to_string(),
        }
    }

    pub fn error(status: StatusCode, detail: &str) -> Self {
        Self {
            status,
            body: json!({ "error_code": "TEST", "detail": detail }).to_string(),
        }
    }
}

/// One request the mock received.
///
/// This module is compiled into every test binary, and each one uses a
/// different subset of the fields.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Recorded {
    pub path: String,
    pub authorization: Option<String>,
    pub body: Option<Value>,
}

#[derive(Default)]
struct Inner {
    replies: HashMap<String, Reply>,
    requests: Vec<Recorded>,
}

#[derive(Clone, Default)]
pub struct MockApi {
    inner: Arc<Mutex<Inner>>,
}

impl MockApi {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the answer for one path. Paths are the ones the server calls, e.g.
    /// `/me`, `/search`, `/persona`.
    pub fn reply(&self, path: &str, reply: Reply) -> &Self {
        self.inner
            .lock()
            .expect("mock state is not poisoned")
            .replies
            .insert(path.to_string(), reply);
        self
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.inner
            .lock()
            .expect("mock state is not poisoned")
            .requests
            .clone()
    }

    /// The recorded requests for one path, in order.
    pub fn requests_for(&self, path: &str) -> Vec<Recorded> {
        self.requests()
            .into_iter()
            .filter(|request| request.path == path)
            .collect()
    }

    fn record(&self, path: &str, headers: &HeaderMap, body: Option<Value>) {
        let authorization = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        self.inner
            .lock()
            .expect("mock state is not poisoned")
            .requests
            .push(Recorded {
                path: path.to_string(),
                authorization,
                body,
            });
    }

    fn reply_for(&self, path: &str) -> Reply {
        self.inner
            .lock()
            .expect("mock state is not poisoned")
            .replies
            .get(path)
            .cloned()
            .unwrap_or_else(|| Reply::raw(StatusCode::NOT_FOUND, r#"{"detail":"no stub"}"#))
    }

    /// Bind on an ephemeral port and serve until the process ends.
    pub async fn spawn(self) -> String {
        let app = Router::new()
            .route("/me", get(handle_get))
            .route("/manage/indexed-sources", get(handle_get))
            .route("/manage/document-set", get(handle_get))
            .route("/persona", get(handle_get))
            .route("/search", post(handle_post))
            .route("/web-search/search-lite", post(handle_post))
            .route("/web-search/open-urls", post(handle_post))
            .with_state(self);

        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("the mock API binds");
        let address = listener.local_addr().expect("the listener has an address");

        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        format!("http://{address}")
    }
}

async fn handle_get(
    State(mock): State<MockApi>,
    uri: axum::http::Uri,
    headers: HeaderMap,
) -> Response {
    let path = uri.path().to_string();
    mock.record(&path, &headers, None);
    into_response(mock.reply_for(&path))
}

async fn handle_post(
    State(mock): State<MockApi>,
    uri: axum::http::Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path = uri.path().to_string();
    mock.record(&path, &headers, serde_json::from_slice(&body).ok());
    into_response(mock.reply_for(&path))
}

fn into_response(reply: Reply) -> Response {
    (
        reply.status,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        reply.body,
    )
        .into_response()
}
