//! The three MCP tools.
//!
//! Port of `backend/lumen/mcp_server/tools/search.py`. The tool descriptions are
//! reproduced verbatim from the Python docstrings: MCP clients read them to
//! decide when and how to call each tool, so a reworded description changes
//! behaviour just as much as reworded code.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::auth::AccessToken;
use crate::metrics::{ToolCallStatus, ToolName, METRICS, UNKNOWN_SOURCE_LABEL};
use crate::resources::{get_accessible_agents, get_indexed_sources, AgentEntry};
use crate::time_cutoff::TimeCutoff;
use crate::upstream::ApiClient;

/// `DocumentSource` values, exported from the Python enum.
///
/// Regenerate with
/// `backend/native/lumen-mcp-server/scripts/export_document_sources.py`;
/// `test_document_sources_export.py` fails if this copy goes stale.
static DOCUMENT_SOURCES: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<String>>(include_str!("../data/document_sources.json"))
        .expect("the exported document sources are valid JSON")
        .into_iter()
        .collect()
});

/// Keeps the unknown-agent error readable when a tenant has many agents.
const MAX_AGENT_NAMES_IN_ERROR: usize = 50;

pub const SEARCH_INDEXED_DOCUMENTS_DESCRIPTION: &str = r#"Search the user's knowledge base indexed in Lumen.
Use this tool for information that is not public knowledge and specific to the user,
their team, their work, or their organization/company.

Runs the full Lumen search pipeline (LLM query expansion, hybrid retrieval,
document selection, context expansion) — the same search quality as the
Lumen chat interface.

To find a list of available sources, use the `indexed_sources` resource.
`document_set_names` restricts results to documents belonging to the named
Document Sets — useful for scoping queries to a curated subset of the
knowledge base (e.g. to isolate knowledge between agents). Use the
`document_sets` resource to discover accessible set names.
`time_cutoff` accepts an ISO 8601 timestamp; only documents updated on or
after that moment are returned. Naive (timezone-less) timestamps are
treated as UTC server-side.
`skip_query_expansion` bypasses the LLM query-expansion step; useful when
you already know the exact phrase to search for (faster, no LLM call for
expansion).
`agent` runs the search as a named Lumen agent, applying that agent's
knowledge scope (its document sets, attached documents and start date) and
its configured model. Pass the name the user gave you — no lookup call is
needed first. If the name does not resolve, the error names the agents
available to this user, so you can retry with a valid one. Use the `agents`
resource to browse them. `agent` and `document_set_names` are mutually
exclusive: explicit document sets replace an agent's knowledge scope rather
than narrowing it, so passing both is rejected.

Returns ``{"results": [{title, url, source_type, content, updated_at},
...]}``. Results are ordered by LLM-judged relevance. ``content`` is the
full chunk of the document the LLM selected; in the rare case the LLM
selection step yields no full chunk for a doc, it falls back to the
short search blurb.

Example usage:
```
{
    "query": "What is the latest status of PROJ-1234 and what is the next development item?",
    "source_types": ["jira", "google_drive", "github"],
    "document_set_names": ["Engineering Wiki"],
    "time_cutoff": "2025-11-24T00:00:00Z",
}
```

Scoping the same question to an agent instead:
```
{
    "query": "What is the latest status of PROJ-1234?",
    "agent": "Engineering Support",
}
```"#;

pub const SEARCH_WEB_DESCRIPTION: &str = r#"Search the public internet for general knowledge, current events, and publicly available information.
Use this tool for information that is publicly available on the web,
such as news, documentation, general facts, or when the user's private knowledge base doesn't contain relevant information.

Returns web search results with titles, URLs, and snippets (NOT full content). Use `open_urls` to fetch full page content.

Example usage:
```
{
    "query": "React 19 migration guide to use react compiler",
    "limit": 5
}
```"#;

pub const OPEN_URLS_DESCRIPTION: &str = r#"Retrieve the complete text content from specific web URLs.
Use this tool when you need to access full content from known URLs,
such as documentation pages or articles returned by the `search_web` tool.

Useful for following up on web search results when snippets do not provide enough information.

Returns the full text content of each URL along with metadata like title and content type.

Example usage:
```
{
    "urls": ["https://react.dev/versions", "https://react.dev/learn/react-compiler","https://react.dev/learn/react-compiler/introduction"]
}
```"#;

/// Arguments of `search_indexed_documents`.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SearchIndexedDocumentsArgs {
    pub query: String,
    #[serde(default)]
    pub source_types: Option<Vec<String>>,
    #[serde(default)]
    pub document_set_names: Option<Vec<String>>,
    #[serde(default)]
    pub time_cutoff: Option<String>,
    #[serde(default)]
    pub skip_query_expansion: bool,
    #[serde(default)]
    pub agent: Option<String>,
}

/// Body of `POST /search`.
///
/// The Python side builds `SearchRequest` with exactly these six fields and
/// serializes with `exclude_unset=True`, so the unset fields (tags, provider,
/// model, message_history) never appear on the wire — and the six below always
/// do, `null` included.
#[derive(Debug, Serialize)]
struct SearchRequestBody {
    query: String,
    sources: Option<Vec<String>>,
    document_sets: Option<Vec<String>>,
    time_cutoff: Option<String>,
    skip_query_expansion: bool,
    persona_id: Option<i64>,
}

/// One result of `POST /search`.
#[derive(Debug, Deserialize)]
struct SearchResult {
    title: String,
    content: String,
    link: Option<String>,
    source_type: String,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResponseBody {
    results: Vec<SearchResult>,
}

/// Convert an API result into the shape MCP clients receive.
///
/// `link` becomes `url`, matching the convention other search MCP tools use.
fn to_mcp_result(result: SearchResult) -> Value {
    json!({
        "title": result.title,
        "url": result.link,
        "source_type": result.source_type,
        "content": result.content,
        "updated_at": result.updated_at,
    })
}

/// The standard MCP error envelope every tool returns.
pub fn error_payload(error: impl Into<String>) -> Value {
    json!({ "error": error.into(), "results": [] })
}

/// Match an agent by name, preferring an exact hit over a case-folded one.
pub fn match_agents<'a>(agent: &str, agents: &'a [AgentEntry]) -> Vec<&'a AgentEntry> {
    let exact: Vec<&AgentEntry> = agents.iter().filter(|entry| entry.name == agent).collect();
    if !exact.is_empty() {
        return exact;
    }
    let folded = agent.to_lowercase();
    agents
        .iter()
        .filter(|entry| entry.name.to_lowercase() == folded)
        .collect()
}

/// Name the valid agents so the caller can retry without a lookup call.
///
/// An unresolvable agent has to fail rather than fall back to an unscoped
/// search: the wrong scope returned as if it were right is worse than an error.
pub fn unknown_agent_error(agent: &str, agents: &[AgentEntry]) -> String {
    if agents.is_empty() {
        return format!("Agent '{agent}' not found. No agents are accessible to this user.");
    }

    let mut names: Vec<&str> = agents.iter().map(|entry| entry.name.as_str()).collect();
    names.sort_unstable();

    let shown = names.len().min(MAX_AGENT_NAMES_IN_ERROR);
    let suffix = if names.len() > shown {
        format!(" (and {} more)", names.len() - shown)
    } else {
        String::new()
    };

    format!(
        "Agent '{agent}' not found. Available agents: {}{suffix}.",
        names[..shown].join(", ")
    )
}

/// Record which source types the caller asked for, folding unknown ones into a
/// single label so the metric's cardinality stays bounded.
fn record_requested_sources(source_types: Option<&Vec<String>>) {
    let mut canonical: BTreeSet<String> = BTreeSet::new();
    for source in source_types.into_iter().flatten() {
        let lowered = source.to_lowercase();
        if DOCUMENT_SOURCES.contains(&lowered) {
            canonical.insert(lowered);
        } else {
            canonical.insert(UNKNOWN_SOURCE_LABEL.to_string());
        }
    }
    for source in canonical {
        METRICS.record_search_source(&source);
    }
}

/// Keep the source types that name a real `DocumentSource`, dropping the rest
/// with a warning — the Python loop does the same.
fn valid_source_types(source_types: &[String]) -> Vec<String> {
    source_types
        .iter()
        .filter_map(|source| {
            let lowered = source.to_lowercase();
            if DOCUMENT_SOURCES.contains(&lowered) {
                Some(lowered)
            } else {
                tracing::warn!(
                    source = %source,
                    "Lumen MCP Server: Invalid source type - skipping"
                );
                None
            }
        })
        .collect()
}

/// Normalize an empty list to `None`.
///
/// `BaseFilters` reads `[]` as "match zero", which is not the same as "no
/// filter", so an empty list must not reach the request body.
fn non_empty(values: Option<Vec<String>>) -> Option<Vec<String>> {
    values.filter(|list| !list.is_empty())
}

pub async fn search_indexed_documents(
    client: &ApiClient,
    token: &AccessToken,
    args: SearchIndexedDocumentsArgs,
) -> Value {
    let started = Instant::now();
    let tool = ToolName::SearchIndexedDocuments;
    let mut status = ToolCallStatus::Error;
    let mut result_count: Option<usize> = None;

    let outcome = run_search(client, token, args, &mut status, &mut result_count).await;

    METRICS.record_tool_outcome(tool, started.elapsed().as_secs_f64(), status);
    if let Some(count) = result_count {
        METRICS.record_search_results(tool, count);
    }
    outcome
}

async fn run_search(
    client: &ApiClient,
    token: &AccessToken,
    args: SearchIndexedDocumentsArgs,
    status: &mut ToolCallStatus,
    result_count: &mut Option<usize>,
) -> Value {
    tracing::info!(
        query = %args.query,
        sources = ?args.source_types,
        document_sets = ?args.document_set_names,
        agent = ?args.agent,
        "Lumen MCP Server: document search"
    );

    record_requested_sources(args.source_types.as_ref());

    let source_types = non_empty(args.source_types);
    let document_set_names = non_empty(args.document_set_names);
    let agent = args
        .agent
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty());

    // `_build_index_filters` lets explicit document sets *replace* an agent's
    // own sets rather than narrow them, so honouring both would silently search
    // outside the agent's knowledge scope.
    if agent.is_some() && document_set_names.is_some() {
        return error_payload(
            "Pass either `agent` or `document_set_names`, not both. Explicit \
             document sets replace an agent's knowledge scope instead of \
             narrowing it, so the results would not be scoped to the agent.",
        );
    }

    // Resolve the agent before the indexed-sources guard: a bad name deserves
    // its own actionable error, and an agent can carry attached documents even
    // when no connector has indexed anything.
    let mut persona_id: Option<i64> = None;
    if let Some(agent) = agent.as_deref() {
        let accessible = match get_accessible_agents(client, token).await {
            Ok(agents) => agents,
            Err(err) => {
                tracing::error!(error = %err, "Lumen MCP Server: Error fetching agents");
                return error_payload(format!("Failed to look up agents: {err}"));
            }
        };

        let matches = match_agents(agent, &accessible);
        match matches.len() {
            0 => return error_payload(unknown_agent_error(agent, &accessible)),
            1 => persona_id = Some(matches[0].id),
            count => {
                return error_payload(format!(
                    "Agent name '{agent}' is ambiguous: {count} accessible \
                     agents share it. Ask the user which one they mean."
                ))
            }
        }
    } else {
        let sources = match get_indexed_sources(client, token).await {
            Ok(sources) => sources,
            Err(err) => {
                tracing::error!(error = %err, "Lumen MCP Server: Error checking indexed sources");
                return error_payload(format!("Failed to check indexed sources: {err}"));
            }
        };

        if sources.is_empty() {
            tracing::info!("Lumen MCP Server: No indexed sources available for tenant");
            *status = ToolCallStatus::Success;
            *result_count = Some(0);
            return error_payload(
                "No document sources are indexed yet. Add connectors or upload data \
                 through Lumen before calling search_indexed_documents.",
            );
        }
    }

    let sources = source_types.as_deref().map(valid_source_types);

    // An unparseable cutoff drops the filter rather than failing the search.
    let parsed_cutoff = args.time_cutoff.as_deref().and_then(|raw| {
        let parsed = TimeCutoff::parse(raw);
        if parsed.is_none() {
            tracing::warn!(
                time_cutoff = %raw,
                "Lumen MCP Server: invalid time_cutoff; continuing without time filter"
            );
        }
        parsed
    });

    let body = SearchRequestBody {
        query: args.query,
        sources,
        document_sets: document_set_names,
        time_cutoff: parsed_cutoff.map(|cutoff| cutoff.to_pydantic_json()),
        skip_query_expansion: args.skip_query_expansion,
        persona_id,
    };

    let payload: SearchResponseBody =
        match client.post_json_for("/search", &token.token, &body).await {
            Ok(payload) => payload,
            Err(err) => {
                if err.status().is_some() {
                    return error_payload(err.to_string());
                }
                tracing::error!(error = %err, "Lumen MCP Server: Document search error");
                return error_payload(format!("Document search failed: {err}"));
            }
        };

    let results: Vec<Value> = payload.results.into_iter().map(to_mcp_result).collect();
    *status = ToolCallStatus::Success;
    *result_count = Some(results.len());
    tracing::info!(
        results = results.len(),
        "Lumen MCP Server: Internal search returned results"
    );
    json!({ "results": results })
}

#[derive(Debug, Deserialize)]
pub struct SearchWebArgs {
    pub query: String,
    #[serde(default = "default_web_limit")]
    pub limit: i64,
}

const fn default_web_limit() -> i64 {
    5
}

#[derive(Debug, Serialize)]
struct WebSearchRequestBody {
    queries: Vec<String>,
    max_results: i64,
}

/// One web-search result, as MCP clients receive it.
///
/// The Python tool validates the API payload through `LlmWebSearchResult` and
/// re-dumps it. Pydantic ignores unknown fields, so anything the API adds is
/// dropped on the way out — modelling the shape here reproduces that projection
/// instead of passing the payload through untouched.
#[derive(Debug, Deserialize, Serialize)]
struct WebSearchResult {
    document_citation_number: i64,
    #[serde(default)]
    unique_identifier_to_strip_away: Option<String>,
    #[serde(default = "web_search_type")]
    r#type: String,
    url: String,
    title: String,
    snippet: String,
}

fn web_search_type() -> String {
    "web_search".to_string()
}

/// One opened-URL result, projected through `LlmOpenUrlResult`.
#[derive(Debug, Deserialize, Serialize)]
struct OpenUrlResult {
    document_citation_number: i64,
    #[serde(default)]
    unique_identifier_to_strip_away: Option<String>,
    #[serde(default = "open_url_type")]
    r#type: String,
    content: String,
}

fn open_url_type() -> String {
    "open_url".to_string()
}

#[derive(Debug, Deserialize)]
struct WebSearchResponseBody {
    results: Vec<WebSearchResult>,
}

#[derive(Debug, Deserialize)]
struct OpenUrlsResponseBody {
    results: Vec<OpenUrlResult>,
}

pub async fn search_web(client: &ApiClient, token: &AccessToken, args: SearchWebArgs) -> Value {
    let started = Instant::now();
    let tool = ToolName::SearchWeb;
    let mut status = ToolCallStatus::Error;
    let mut result_count: Option<usize> = None;

    tracing::info!(
        query = %args.query,
        limit = args.limit,
        "Lumen MCP Server: Web search"
    );

    let body = WebSearchRequestBody {
        queries: vec![args.query.clone()],
        max_results: args.limit,
    };

    let outcome = match client
        .post_json_for::<_, WebSearchResponseBody>("/web-search/search-lite", &token.token, &body)
        .await
    {
        Ok(payload) => {
            status = ToolCallStatus::Success;
            result_count = Some(payload.results.len());
            json!({ "results": payload.results, "query": args.query })
        }
        Err(err) => {
            let message = if err.status().is_some() {
                err.to_string()
            } else {
                tracing::error!(error = %err, "Lumen MCP Server: Web search error");
                format!("Web search failed: {err}")
            };
            json!({ "error": message, "results": [], "query": args.query })
        }
    };

    METRICS.record_tool_outcome(tool, started.elapsed().as_secs_f64(), status);
    if let Some(count) = result_count {
        METRICS.record_search_results(tool, count);
    }
    outcome
}

#[derive(Debug, Deserialize)]
pub struct OpenUrlsArgs {
    pub urls: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OpenUrlsRequestBody {
    urls: Vec<String>,
}

pub async fn open_urls(client: &ApiClient, token: &AccessToken, args: OpenUrlsArgs) -> Value {
    let started = Instant::now();
    let tool = ToolName::OpenUrls;
    let mut status = ToolCallStatus::Error;

    tracing::info!(
        urls = args.urls.len(),
        "Lumen MCP Server: Open URL: fetching URLs"
    );

    let body = OpenUrlsRequestBody { urls: args.urls };

    let outcome = match client
        .post_json_for::<_, OpenUrlsResponseBody>("/web-search/open-urls", &token.token, &body)
        .await
    {
        Ok(payload) => {
            status = ToolCallStatus::Success;
            json!({ "results": payload.results })
        }
        Err(err) => {
            if err.status().is_some() {
                error_payload(err.to_string())
            } else {
                tracing::error!(error = %err, "Lumen MCP Server: URL fetch error");
                error_payload(format!("URL fetch failed: {err}"))
            }
        }
    };

    // `open_urls` records no result count; the Python tool does not either.
    METRICS.record_tool_outcome(tool, started.elapsed().as_secs_f64(), status);
    outcome
}

/// Argument schemas, exported from the Python server.
///
/// FastMCP derives these from the Python signatures through Pydantic, which
/// emits shapes a hand-written schema does not match (`anyOf` for optionals,
/// `additionalProperties: false`). MCP clients read them to decide how to call
/// a tool, so they are exported rather than rewritten. Regenerate with
/// `scripts/export_contracts.py`; `test_exported_contracts.py` catches drift.
static TOOL_SCHEMAS: LazyLock<BTreeMap<String, Map<String, Value>>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/tool_schemas.json"))
        .expect("the exported tool schemas are valid JSON")
});

fn schema_for(tool: &str) -> Map<String, Value> {
    TOOL_SCHEMAS
        .get(tool)
        .cloned()
        .unwrap_or_else(|| panic!("no exported schema for {tool}"))
}

pub fn search_indexed_documents_schema() -> Map<String, Value> {
    schema_for("search_indexed_documents")
}

pub fn search_web_schema() -> Map<String, Value> {
    schema_for("search_web")
}

pub fn open_urls_schema() -> Map<String, Value> {
    schema_for("open_urls")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(id: i64, name: &str) -> AgentEntry {
        AgentEntry {
            id,
            name: name.to_string(),
            description: None,
        }
    }

    #[test]
    fn an_exact_name_beats_a_case_folded_one() {
        let agents = vec![agent(1, "Support"), agent(2, "support")];
        let matches = match_agents("Support", &agents);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, 1);
    }

    #[test]
    fn case_folding_applies_only_without_an_exact_hit() {
        let agents = vec![agent(2, "support")];
        let matches = match_agents("SUPPORT", &agents);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, 2);
    }

    #[test]
    fn a_shared_folded_name_reports_every_match() {
        let agents = vec![agent(1, "support"), agent(2, "Support ")];
        assert_eq!(match_agents("SUPPORT", &agents).len(), 1);

        let agents = vec![agent(1, "support"), agent(2, "SUPPORT")];
        assert_eq!(match_agents("Support", &agents).len(), 2);
    }

    #[test]
    fn unknown_agent_error_names_the_alternatives_in_order() {
        let agents = vec![agent(1, "Zeta"), agent(2, "Alpha")];
        assert_eq!(
            unknown_agent_error("Missing", &agents),
            "Agent 'Missing' not found. Available agents: Alpha, Zeta."
        );
    }

    #[test]
    fn unknown_agent_error_handles_an_empty_roster() {
        assert_eq!(
            unknown_agent_error("Missing", &[]),
            "Agent 'Missing' not found. No agents are accessible to this user."
        );
    }

    #[test]
    fn unknown_agent_error_truncates_a_long_roster() {
        let agents: Vec<AgentEntry> = (0..60)
            .map(|index| agent(index, &format!("Agent{index:02}")))
            .collect();
        let message = unknown_agent_error("Missing", &agents);
        assert!(message.ends_with(" (and 10 more)."), "{message}");
        assert!(message.contains("Agent00"));
        assert!(!message.contains("Agent59"));
    }

    #[test]
    fn invalid_source_types_are_dropped_and_lowercased() {
        let sources = vec![
            "JIRA".to_string(),
            "not_a_source".to_string(),
            "github".to_string(),
        ];
        assert_eq!(valid_source_types(&sources), vec!["jira", "github"]);
    }

    #[test]
    fn empty_lists_normalize_to_none() {
        assert_eq!(non_empty(Some(vec![])), None);
        assert_eq!(non_empty(None), None);
        assert_eq!(
            non_empty(Some(vec!["a".to_string()])),
            Some(vec!["a".to_string()])
        );
    }

    #[test]
    fn the_search_body_carries_exactly_the_six_set_fields() {
        let body = SearchRequestBody {
            query: "q".to_string(),
            sources: None,
            document_sets: None,
            time_cutoff: None,
            skip_query_expansion: false,
            persona_id: None,
        };
        let value: Value = serde_json::to_value(&body).unwrap();
        // Key order is irrelevant on the wire; the set of keys is not.
        let mut keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "document_sets",
                "persona_id",
                "query",
                "skip_query_expansion",
                "sources",
                "time_cutoff",
            ]
        );
        // The fields Python leaves unset must never appear.
        for absent in ["tags", "provider", "model", "message_history"] {
            assert!(value.get(absent).is_none(), "{absent} leaked into the body");
        }
    }

    #[test]
    fn a_result_renames_link_to_url() {
        let result = SearchResult {
            title: "T".to_string(),
            content: "C".to_string(),
            link: Some("https://example.com".to_string()),
            source_type: "jira".to_string(),
            updated_at: None,
        };
        let value = to_mcp_result(result);
        assert_eq!(value["url"], "https://example.com");
        assert!(value.get("link").is_none());
        assert_eq!(value["updated_at"], Value::Null);
    }

    #[test]
    fn the_error_envelope_always_carries_empty_results() {
        assert_eq!(error_payload("boom"), json!({"error":"boom","results":[]}));
    }

    #[test]
    fn every_exported_document_source_loads() {
        assert!(DOCUMENT_SOURCES.contains("jira"));
        assert!(DOCUMENT_SOURCES.contains("google_drive"));
        assert!(!DOCUMENT_SOURCES.contains("not_a_source"));
    }

    #[test]
    fn the_exported_schemas_cover_every_tool() {
        for tool in ["search_indexed_documents", "search_web", "open_urls"] {
            let schema = schema_for(tool);
            assert_eq!(schema["type"], "object", "{tool}");
            // Pydantic emits this; a hand-written schema would not.
            assert_eq!(schema["additionalProperties"], false, "{tool}");
            assert!(schema.contains_key("properties"), "{tool}");
        }
    }

    #[test]
    fn web_search_limit_defaults_to_five() {
        let args: SearchWebArgs = serde_json::from_str(r#"{"query":"q"}"#).unwrap();
        assert_eq!(args.limit, 5);
    }

    #[test]
    fn web_results_drop_fields_the_python_model_does_not_declare() {
        // Pydantic ignores unknown fields on validation and never re-emits them.
        let raw = r#"{
            "document_citation_number": 1,
            "unique_identifier_to_strip_away": null,
            "type": "web_search",
            "url": "https://example.com",
            "title": "T",
            "snippet": "S",
            "provider_internal_score": 0.9
        }"#;
        let result: WebSearchResult = serde_json::from_str(raw).unwrap();
        let value = serde_json::to_value(&result).unwrap();
        assert!(value.get("provider_internal_score").is_none());

        let mut keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "document_citation_number",
                "snippet",
                "title",
                "type",
                "unique_identifier_to_strip_away",
                "url",
            ]
        );
    }

    #[test]
    fn open_url_results_keep_only_the_declared_fields() {
        let raw = r#"{
            "document_citation_number": 2,
            "type": "open_url",
            "content": "body",
            "fetched_with": "playwright"
        }"#;
        let result: OpenUrlResult = serde_json::from_str(raw).unwrap();
        let value = serde_json::to_value(&result).unwrap();
        assert!(value.get("fetched_with").is_none());
        // A field with a Python default is still emitted, as null.
        assert_eq!(value["unique_identifier_to_strip_away"], Value::Null);
        assert_eq!(value["type"], "open_url");
    }

    #[test]
    fn a_result_missing_a_required_field_fails_to_parse() {
        // Pydantic would raise here too, and the tool turns that into an error
        // envelope rather than a partial result.
        let raw = r#"{"type":"web_search","url":"u","title":"t","snippet":"s"}"#;
        assert!(serde_json::from_str::<WebSearchResult>(raw).is_err());
    }
}
