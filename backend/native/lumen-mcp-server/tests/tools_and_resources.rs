//! Tool and resource behaviour against a mock Lumen API.
//!
//! These drive the ported functions directly, so every branch of the Python
//! original gets an assertion without going through the MCP protocol. The
//! protocol wiring is covered separately in `mcp_protocol.rs`.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use common::mock_api::{MockApi, Reply};
use lumen_mcp_server::auth::AccessToken;
use lumen_mcp_server::resources;
use lumen_mcp_server::tools::{self, OpenUrlsArgs, SearchIndexedDocumentsArgs, SearchWebArgs};
use lumen_mcp_server::upstream::ApiClient;
use serde_json::json;

const TOKEN: &str = "test-token";

async fn setup() -> (MockApi, ApiClient, AccessToken) {
    let mock = MockApi::new();
    let base_url = mock.clone().spawn().await;
    let client =
        ApiClient::with_base_url(&base_url, Duration::from_secs(5)).expect("client builds");
    (
        mock,
        client,
        AccessToken {
            token: TOKEN.to_string(),
        },
    )
}

fn search_args(query: &str) -> SearchIndexedDocumentsArgs {
    SearchIndexedDocumentsArgs {
        query: query.to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn indexed_sources_are_sorted_and_stringified() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/manage/indexed-sources",
        Reply::ok(json!({"sources": ["jira", "github", 42]})),
    );

    let body = resources::indexed_sources_body(&client, &token)
        .await
        .expect("the resource resolves");

    assert_eq!(body, r#"["42","github","jira"]"#);
    assert_eq!(
        mock.requests_for("/manage/indexed-sources")[0].authorization,
        Some(format!("Bearer {TOKEN}"))
    );
}

#[tokio::test]
async fn document_sets_are_projected_and_sorted_by_name() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/manage/document-set",
        Reply::ok(json!([
            {"id": 2, "name": "Zeta", "description": "z", "cc_pair_summaries": []},
            {"id": 1, "name": "Alpha", "description": null}
        ])),
    );

    let body = resources::document_sets_body(&client, &token)
        .await
        .expect("the resource resolves");

    assert_eq!(
        body,
        r#"[{"name":"Alpha","description":null},{"name":"Zeta","description":"z"}]"#
    );
}

#[tokio::test]
async fn agents_are_projected_and_sorted_by_name() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/persona",
        Reply::ok(json!([
            {"id": 9, "name": "Support", "description": "help", "tools": []},
            {"id": 3, "name": "Analytics", "description": null}
        ])),
    );

    let body = resources::agents_body(&client, &token)
        .await
        .expect("the resource resolves");

    assert_eq!(
        body,
        r#"[{"id":3,"name":"Analytics","description":null},{"id":9,"name":"Support","description":"help"}]"#
    );
}

#[tokio::test]
async fn a_failing_resource_surfaces_the_upstream_detail() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/persona",
        Reply::error(StatusCode::FORBIDDEN, "No access to agents"),
    );

    let error = resources::agents_body(&client, &token)
        .await
        .expect_err("the resource fails");
    assert_eq!(error.to_string(), "No access to agents");
}

#[tokio::test]
async fn a_search_without_indexed_sources_explains_itself() {
    let (mock, client, token) = setup().await;
    mock.reply("/manage/indexed-sources", Reply::ok(json!({"sources": []})));

    let result = tools::search_indexed_documents(&client, &token, search_args("q")).await;

    assert_eq!(result["results"], json!([]));
    assert_eq!(
        result["error"],
        "No document sources are indexed yet. Add connectors or upload data \
         through Lumen before calling search_indexed_documents."
    );
    // The guard short-circuits: /search is never called.
    assert!(mock.requests_for("/search").is_empty());
}

#[tokio::test]
async fn a_search_forwards_filters_and_renames_link_to_url() {
    let (mock, client, token) = setup().await;
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
            "updated_at": "2025-01-01T00:00:00Z"
        }]})),
    );

    let args = SearchIndexedDocumentsArgs {
        query: "status".to_string(),
        source_types: Some(vec!["JIRA".to_string(), "bogus".to_string()]),
        time_cutoff: Some("2025-11-24T00:00:00Z".to_string()),
        skip_query_expansion: true,
        ..Default::default()
    };
    let result = tools::search_indexed_documents(&client, &token, args).await;

    assert_eq!(result["results"][0]["url"], "https://jira/PROJ-1");
    assert!(result["results"][0].get("link").is_none());

    let sent = mock.requests_for("/search")[0]
        .body
        .clone()
        .expect("the search body is JSON");
    // The unknown source is dropped, the known one lowercased.
    assert_eq!(sent["sources"], json!(["jira"]));
    assert_eq!(sent["time_cutoff"], "2025-11-24T00:00:00Z");
    assert_eq!(sent["skip_query_expansion"], true);
    assert_eq!(sent["persona_id"], json!(null));
    assert_eq!(sent["document_sets"], json!(null));
}

#[tokio::test]
async fn an_unparseable_time_cutoff_drops_the_filter_instead_of_failing() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/manage/indexed-sources",
        Reply::ok(json!({"sources": ["jira"]})),
    );
    mock.reply("/search", Reply::ok(json!({"results": []})));

    let args = SearchIndexedDocumentsArgs {
        query: "q".to_string(),
        time_cutoff: Some("not-a-date".to_string()),
        ..Default::default()
    };
    let result = tools::search_indexed_documents(&client, &token, args).await;

    assert_eq!(result["results"], json!([]));
    let sent = mock.requests_for("/search")[0].body.clone().unwrap();
    assert_eq!(sent["time_cutoff"], json!(null));
}

#[tokio::test]
async fn an_agent_and_document_sets_together_are_rejected() {
    let (mock, client, token) = setup().await;

    let args = SearchIndexedDocumentsArgs {
        query: "q".to_string(),
        document_set_names: Some(vec!["Wiki".to_string()]),
        agent: Some("Support".to_string()),
        ..Default::default()
    };
    let result = tools::search_indexed_documents(&client, &token, args).await;

    assert!(result["error"]
        .as_str()
        .expect("an error string")
        .starts_with("Pass either `agent` or `document_set_names`, not both."));
    // Neither lookup runs: the guard fires first.
    assert!(mock.requests_for("/persona").is_empty());
    assert!(mock.requests_for("/manage/indexed-sources").is_empty());
}

#[tokio::test]
async fn a_named_agent_resolves_to_a_persona_id() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/persona",
        Reply::ok(json!([{"id": 7, "name": "Support", "description": null}])),
    );
    mock.reply("/search", Reply::ok(json!({"results": []})));

    let args = SearchIndexedDocumentsArgs {
        query: "q".to_string(),
        agent: Some("  support  ".to_string()),
        ..Default::default()
    };
    let result = tools::search_indexed_documents(&client, &token, args).await;

    assert_eq!(result["results"], json!([]));
    let sent = mock.requests_for("/search")[0].body.clone().unwrap();
    assert_eq!(sent["persona_id"], 7);
    // With an agent, the indexed-sources guard is skipped.
    assert!(mock.requests_for("/manage/indexed-sources").is_empty());
}

#[tokio::test]
async fn an_unknown_agent_names_the_available_ones() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/persona",
        Reply::ok(json!([
            {"id": 1, "name": "Alpha", "description": null},
            {"id": 2, "name": "Beta", "description": null}
        ])),
    );

    let args = SearchIndexedDocumentsArgs {
        query: "q".to_string(),
        agent: Some("Missing".to_string()),
        ..Default::default()
    };
    let result = tools::search_indexed_documents(&client, &token, args).await;

    assert_eq!(
        result["error"],
        "Agent 'Missing' not found. Available agents: Alpha, Beta."
    );
}

#[tokio::test]
async fn an_ambiguous_agent_asks_the_user_to_choose() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/persona",
        Reply::ok(json!([
            {"id": 1, "name": "support", "description": null},
            {"id": 2, "name": "SUPPORT", "description": null}
        ])),
    );

    let args = SearchIndexedDocumentsArgs {
        query: "q".to_string(),
        agent: Some("Support".to_string()),
        ..Default::default()
    };
    let result = tools::search_indexed_documents(&client, &token, args).await;

    assert_eq!(
        result["error"],
        "Agent name 'Support' is ambiguous: 2 accessible agents share it. \
         Ask the user which one they mean."
    );
}

#[tokio::test]
async fn a_failing_search_returns_the_upstream_detail() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/manage/indexed-sources",
        Reply::ok(json!({"sources": ["jira"]})),
    );
    mock.reply(
        "/search",
        Reply::error(StatusCode::BAD_REQUEST, "query too long"),
    );

    let result = tools::search_indexed_documents(&client, &token, search_args("q")).await;
    assert_eq!(result["error"], "query too long");
    assert_eq!(result["results"], json!([]));
}

#[tokio::test]
async fn a_non_json_search_error_falls_back_to_the_status_line() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/manage/indexed-sources",
        Reply::ok(json!({"sources": ["jira"]})),
    );
    mock.reply(
        "/search",
        Reply::raw(StatusCode::BAD_GATEWAY, "<html>nginx</html>"),
    );

    let result = tools::search_indexed_documents(&client, &token, search_args("q")).await;
    assert_eq!(result["error"], "Request failed with status 502");
}

#[tokio::test]
async fn web_search_passes_the_query_and_limit_through() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/web-search/search-lite",
        Reply::ok(json!({
            "results": [{
                "document_citation_number": 1,
                "unique_identifier_to_strip_away": null,
                "type": "web_search",
                "url": "https://example.com",
                "title": "T",
                "snippet": "S",
                "provider_score": 0.9
            }],
            "provider_type": "exa"
        })),
    );

    let args = SearchWebArgs {
        query: "rust".to_string(),
        limit: 3,
    };
    let result = tools::search_web(&client, &token, args).await;

    assert_eq!(result["query"], "rust");
    assert_eq!(result["results"][0]["url"], "https://example.com");
    // A field the Python model does not declare must not reach the client.
    assert!(result["results"][0].get("provider_score").is_none());

    let sent = mock.requests_for("/web-search/search-lite")[0]
        .body
        .clone()
        .unwrap();
    assert_eq!(sent["queries"], json!(["rust"]));
    assert_eq!(sent["max_results"], 3);
}

#[tokio::test]
async fn a_failing_web_search_keeps_the_query_in_the_envelope() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/web-search/search-lite",
        Reply::error(StatusCode::TOO_MANY_REQUESTS, "rate limited"),
    );

    let args = SearchWebArgs {
        query: "rust".to_string(),
        limit: 5,
    };
    let result = tools::search_web(&client, &token, args).await;

    assert_eq!(result["error"], "rate limited");
    assert_eq!(result["results"], json!([]));
    assert_eq!(result["query"], "rust");
}

#[tokio::test]
async fn open_urls_forwards_the_url_list() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/web-search/open-urls",
        Reply::ok(json!({
            "results": [{
                "document_citation_number": 1,
                "type": "open_url",
                "content": "page body",
                "fetched_with": "playwright"
            }],
            "provider_type": null
        })),
    );

    let args = OpenUrlsArgs {
        urls: vec!["https://a".to_string(), "https://b".to_string()],
    };
    let result = tools::open_urls(&client, &token, args).await;

    assert_eq!(result["results"][0]["content"], "page body");
    assert!(result["results"][0].get("fetched_with").is_none());

    let sent = mock.requests_for("/web-search/open-urls")[0]
        .body
        .clone()
        .unwrap();
    assert_eq!(sent["urls"], json!(["https://a", "https://b"]));
}

#[tokio::test]
async fn a_failing_open_urls_uses_the_standard_error_envelope() {
    let (mock, client, token) = setup().await;
    mock.reply(
        "/web-search/open-urls",
        Reply::error(StatusCode::BAD_REQUEST, "bad url"),
    );

    let args = OpenUrlsArgs {
        urls: vec!["not-a-url".to_string()],
    };
    let result = tools::open_urls(&client, &token, args).await;

    assert_eq!(result["error"], "bad url");
    assert_eq!(result["results"], json!([]));
}
