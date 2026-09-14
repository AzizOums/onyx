//! The HTTP surface and the MCP protocol wiring.
//!
//! Runs the real axum app against a mock Lumen API and speaks the real MCP
//! protocol to it, so the transport, the auth middleware and the handler are all
//! exercised the way a client would.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use axum::http::StatusCode;
use common::mock_api::{MockApi, Reply};
use lumen_mcp_server::config::Config;
use lumen_mcp_server::{http, upstream::ApiClient};
use rmcp::model::{CallToolRequestParams, ReadResourceRequestParams};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::ServiceExt;
use serde_json::json;

const GOOD_TOKEN: &str = "good-token";

fn test_config() -> Config {
    Config {
        enabled: true,
        host: "127.0.0.1".to_string(),
        port: 0,
        cors_origins: Vec::new(),
        api_request_timeout: Duration::from_secs(5),
        api_base_url: String::new(),
    }
}

/// Start the mock API and the MCP server. Returns the mock and the server's URL.
async fn start() -> (MockApi, String) {
    let mock = MockApi::new();
    let api_url = mock.clone().spawn().await;
    // Only this token gets a 200 from /me.
    mock.reply("/me", Reply::ok(json!({"id": "user-1"})));

    let client = ApiClient::with_base_url(&api_url, Duration::from_secs(5)).expect("client builds");
    let app = http::build_app(&test_config(), client);

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("the server binds");
    let address = listener.local_addr().expect("the listener has an address");

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    (mock, format!("http://{address}"))
}

async fn connect(url: &str, token: &str) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let transport = StreamableHttpClientTransport::with_client(
        reqwest::Client::default(),
        // rmcp applies `bearer_auth` itself, so this is the bare token.
        StreamableHttpClientTransportConfig::with_uri(url).auth_header(token),
    );
    ().serve(transport)
        .await
        .expect("the MCP handshake succeeds")
}

#[tokio::test]
async fn health_is_public_and_matches_the_python_body() {
    let (_mock, url) = start().await;

    let response = reqwest::get(format!("{url}/health"))
        .await
        .expect("the health endpoint answers");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.text().await.unwrap(),
        r#"{"status":"healthy","service":"mcp_server"}"#
    );
}

#[tokio::test]
async fn metrics_are_public_and_expose_the_prometheus_series() {
    let (_mock, url) = start().await;

    let body = reqwest::get(format!("{url}/metrics"))
        .await
        .expect("the metrics endpoint answers")
        .text()
        .await
        .unwrap();

    // The auth counter is registered at startup, so it is always present.
    assert!(body.contains("lumen_mcp_server_auth_total"), "{body}");
}

#[tokio::test]
async fn a_request_without_a_token_is_rejected() {
    let (_mock, url) = start().await;

    let response = reqwest::Client::new()
        .post(&url)
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
        .send()
        .await
        .expect("the server answers");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().contains_key("www-authenticate"));
}

#[tokio::test]
async fn a_token_the_api_rejects_is_rejected_here_too() {
    let (mock, url) = start().await;
    mock.reply("/me", Reply::error(StatusCode::UNAUTHORIZED, "bad token"));

    let response = reqwest::Client::new()
        .post(&url)
        .bearer_auth("stale-token")
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
        .send()
        .await
        .expect("the server answers");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    // The token was actually checked against the API.
    assert_eq!(
        mock.requests_for("/me")[0].authorization,
        Some("Bearer stale-token".to_string())
    );
}

#[tokio::test]
async fn the_server_lists_the_three_tools_with_their_descriptions() {
    let (_mock, url) = start().await;
    let client = connect(&url, GOOD_TOKEN).await;

    let tools = client
        .list_tools(None)
        .await
        .expect("tools/list works")
        .tools;
    let mut names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec!["open_urls", "search_indexed_documents", "search_web"]
    );

    let search = tools
        .iter()
        .find(|tool| tool.name == "search_indexed_documents")
        .expect("the search tool is listed");
    let description = search.description.as_deref().unwrap_or_default();
    // The description is what an MCP client reads to decide how to call it.
    assert!(description.starts_with("Search the user's knowledge base indexed in Lumen."));
    assert!(description.contains("`agent` and `document_set_names` are mutually"));
    assert!(search.input_schema.contains_key("properties"));

    client.cancel().await.ok();
}

#[tokio::test]
async fn the_server_lists_the_three_resources() {
    let (_mock, url) = start().await;
    let client = connect(&url, GOOD_TOKEN).await;

    let resources = client
        .list_resources(None)
        .await
        .expect("resources/list works")
        .resources;

    let mut uris: Vec<&str> = resources.iter().map(|res| res.uri.as_str()).collect();
    uris.sort_unstable();
    assert_eq!(
        uris,
        vec![
            "resource://agents",
            "resource://document_sets",
            "resource://indexed_sources"
        ]
    );
    for resource in &resources {
        assert_eq!(resource.mime_type.as_deref(), Some("application/json"));
        assert!(resource.description.is_some());
    }

    client.cancel().await.ok();
}

#[tokio::test]
async fn a_tool_call_reaches_the_api_and_returns_structured_content() {
    let (mock, url) = start().await;
    mock.reply(
        "/manage/indexed-sources",
        Reply::ok(json!({"sources": ["jira"]})),
    );
    mock.reply(
        "/search",
        Reply::ok(json!({"results": [{
            "citation_id": 1,
            "title": "Ticket",
            "content": "body",
            "link": "https://jira/PROJ-1",
            "source_type": "jira",
            "updated_at": null
        }]})),
    );

    let client = connect(&url, GOOD_TOKEN).await;
    let result = client
        .call_tool(
            CallToolRequestParams::new("search_indexed_documents").with_arguments(
                json!({"query": "status"})
                    .as_object()
                    .cloned()
                    .expect("an object"),
            ),
        )
        .await
        .expect("the tool call works");

    let structured = result
        .structured_content
        .expect("the tool returns structured content");
    assert_eq!(structured["results"][0]["url"], "https://jira/PROJ-1");

    // The caller's token was forwarded upstream, not a service credential.
    assert_eq!(
        mock.requests_for("/search")[0].authorization,
        Some(format!("Bearer {GOOD_TOKEN}"))
    );

    client.cancel().await.ok();
}

#[tokio::test]
async fn a_resource_read_returns_the_json_body() {
    let (mock, url) = start().await;
    mock.reply(
        "/manage/indexed-sources",
        Reply::ok(json!({"sources": ["jira", "github"]})),
    );

    let client = connect(&url, GOOD_TOKEN).await;
    let result = client
        .read_resource(ReadResourceRequestParams::new("resource://indexed_sources"))
        .await
        .expect("the resource read works");

    let contents = result.contents.first().expect("one content block");
    match contents {
        rmcp::model::ResourceContents::TextResourceContents {
            text, mime_type, ..
        } => {
            assert_eq!(text, r#"["github","jira"]"#);
            assert_eq!(mime_type.as_deref(), Some("application/json"));
        }
        other => panic!("expected text contents, got {other:?}"),
    }

    client.cancel().await.ok();
}

#[tokio::test]
async fn a_tool_error_comes_back_as_an_envelope_not_a_protocol_error() {
    let (mock, url) = start().await;
    mock.reply("/manage/indexed-sources", Reply::ok(json!({"sources": []})));

    let client = connect(&url, GOOD_TOKEN).await;
    let result = client
        .call_tool(
            CallToolRequestParams::new("search_indexed_documents")
                .with_arguments(json!({"query": "q"}).as_object().cloned().unwrap()),
        )
        .await
        .expect("the call completes rather than failing");

    let structured = result.structured_content.expect("structured content");
    assert!(structured["error"]
        .as_str()
        .unwrap()
        .starts_with("No document sources are indexed yet."));

    client.cancel().await.ok();
}
