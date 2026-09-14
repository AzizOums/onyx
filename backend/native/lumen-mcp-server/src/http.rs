//! The HTTP surface.
//!
//! Port of `create_mcp_fastapi_app` in `backend/lumen/mcp_server/api.py`:
//! a public health endpoint, Prometheus metrics, optional CORS, bearer auth,
//! and the MCP streamable-HTTP transport mounted at the root.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

use crate::auth::{bearer_token, verify_token};
use crate::config::Config;
use crate::metrics::METRICS;
use crate::server::LumenMcpHandler;
use crate::upstream::ApiClient;

/// Health payload, byte-identical to the Python endpoint's.
const HEALTH_BODY: &str = r#"{"status":"healthy","service":"mcp_server"}"#;

#[derive(Clone)]
struct AuthState {
    client: ApiClient,
}

async fn health() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        HEALTH_BODY,
    )
        .into_response()
}

async fn metrics() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        METRICS.encode(),
    )
        .into_response()
}

/// Reject anything without a token the Lumen API accepts.
///
/// On success the verified token goes into the request extensions, where the
/// MCP handler reads it back out.
async fn require_bearer(
    State(state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    let header_value = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    let token = header_value.as_deref().and_then(bearer_token);

    let Some(token) = token else {
        return unauthorized("Missing or malformed Authorization header");
    };

    let Some(access_token) = verify_token(&state.client, token).await else {
        return unauthorized("Invalid token");
    };

    request.extensions_mut().insert(access_token);
    next.run(request).await
}

fn unauthorized(detail: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static(r#"Bearer realm="lumen", error="invalid_token""#),
        )],
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        serde_json::json!({ "error": "unauthorized", "detail": detail }).to_string(),
    )
        .into_response()
}

/// Make sure the Accept header carries the types the streamable transport needs.
///
/// Port of `_ensure_streamable_accept_header`: several MCP clients send `*/*`
/// or omit the header, and the transport would otherwise refuse them.
async fn ensure_streamable_accept(mut request: Request, next: Next) -> Response {
    const REQUIRED: &str = "application/json, text/event-stream";

    let accept = request
        .headers()
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();

    let lowered = accept.to_lowercase();
    let needs_rewrite = accept.is_empty()
        || accept == "*/*"
        || !lowered.contains("application/json")
        || !lowered.contains("text/event-stream");

    if needs_rewrite {
        request
            .headers_mut()
            .insert(header::ACCEPT, HeaderValue::from_static(REQUIRED));
    }

    next.run(request).await
}

fn cors_layer(origins: &[String]) -> Option<CorsLayer> {
    if origins.is_empty() {
        return None;
    }

    tracing::info!(origins = ?origins, "CORS origins");

    let parsed: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok())
        .collect();

    Some(
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(parsed))
            .allow_methods(Any)
            .allow_headers(Any),
    )
}

/// Build the whole HTTP application.
pub fn build_app(config: &Config, client: ApiClient) -> Router {
    let handler = LumenMcpHandler::new(client.clone());

    let mcp_service = StreamableHttpService::new(
        move || Ok(handler.clone()),
        Arc::new(LocalSessionManager::default()),
        // The Python server performs no Host validation, so enabling rmcp's
        // default loopback-only check here would reject requests FastMCP
        // accepts. `MCP_SERVER_ALLOWED_HOSTS` opts back in.
        allowed_hosts_config(),
    );

    let auth_state = AuthState { client };

    let mcp_router = Router::new()
        .fallback_service(mcp_service)
        .layer(middleware::from_fn(ensure_streamable_accept))
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            require_bearer,
        ))
        .with_state(auth_state);

    // /health and /metrics are public, exactly as in the Python app.
    let mut app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .merge(mcp_router);

    if let Some(layer) = cors_layer(&config.cors_origins) {
        app = app.layer(layer);
    }

    app
}

fn allowed_hosts_config() -> StreamableHttpServerConfig {
    let config = StreamableHttpServerConfig::default();
    match std::env::var("MCP_SERVER_ALLOWED_HOSTS") {
        Ok(raw) if !raw.trim().is_empty() => {
            let hosts: Vec<String> = raw
                .split(',')
                .map(str::trim)
                .filter(|host| !host.is_empty())
                .map(str::to_string)
                .collect();
            config.with_allowed_hosts(hosts)
        }
        _ => config.disable_allowed_hosts(),
    }
}

/// Serve until the process is asked to stop.
pub async fn serve(config: &Config, client: ApiClient) -> anyhow::Result<()> {
    let app = build_app(config, client);
    let address = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&address).await?;

    tracing::info!(address = %address, "Starting MCP server");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("MCP server shutting down");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_health_body_matches_the_python_endpoint() {
        assert_eq!(
            HEALTH_BODY,
            r#"{"status":"healthy","service":"mcp_server"}"#
        );
    }

    #[test]
    fn cors_is_off_when_no_origin_is_configured() {
        assert!(cors_layer(&[]).is_none());
        assert!(cors_layer(&["https://app.example.com".to_string()]).is_some());
    }
}
