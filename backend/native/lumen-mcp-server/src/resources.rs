//! The three MCP resources.
//!
//! Port of `backend/lumen/mcp_server/resources/`. Each one fetches from the
//! Lumen API, projects the payload down to the fields MCP clients need, sorts
//! it, and returns JSON text. The URIs, names, descriptions and mime type are
//! reproduced verbatim: MCP clients read them to decide what to call.

use serde::{Deserialize, Serialize};

use crate::auth::AccessToken;
use crate::upstream::{ApiClient, UpstreamError};

/// Serialize the way Python's `json.dumps` does by default.
///
/// The Python resources return `json.dumps(...)`, whose default separators are
/// `", "` and `": "`. `serde_json` writes neither space, so the bodies would
/// differ byte for byte even though the data matches. MCP clients receive this
/// text verbatim, so the spacing is part of the contract.
mod python_json {
    use std::io;

    use serde::Serialize;
    use serde_json::ser::Formatter;

    struct PythonFormatter;

    impl Formatter for PythonFormatter {
        fn begin_array_value<W: ?Sized + io::Write>(
            &mut self,
            writer: &mut W,
            first: bool,
        ) -> io::Result<()> {
            if first {
                Ok(())
            } else {
                writer.write_all(b", ")
            }
        }

        fn begin_object_key<W: ?Sized + io::Write>(
            &mut self,
            writer: &mut W,
            first: bool,
        ) -> io::Result<()> {
            if first {
                Ok(())
            } else {
                writer.write_all(b", ")
            }
        }

        fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
            writer.write_all(b": ")
        }
    }

    pub fn to_string<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
        let mut buffer = Vec::with_capacity(128);
        let mut serializer = serde_json::Serializer::with_formatter(&mut buffer, PythonFormatter);
        value.serialize(&mut serializer)?;
        // The formatter only ever writes UTF-8, as serde_json does.
        Ok(String::from_utf8(buffer).expect("serde_json emits UTF-8"))
    }
}

pub const MIME_TYPE: &str = "application/json";

pub const INDEXED_SOURCES_URI: &str = "resource://indexed_sources";
pub const INDEXED_SOURCES_NAME: &str = "indexed_sources";
pub const INDEXED_SOURCES_DESCRIPTION: &str = concat!(
    "Enumerate the user's document sources that are currently indexed in Lumen.",
    "This can be used to discover filters for the `search_indexed_documents` tool."
);

pub const DOCUMENT_SETS_URI: &str = "resource://document_sets";
pub const DOCUMENT_SETS_NAME: &str = "document_sets";
pub const DOCUMENT_SETS_DESCRIPTION: &str = concat!(
    "Enumerate the Document Sets accessible to the current user. Use the ",
    "returned `name` values with the `document_set_names` filter of the ",
    "`search_indexed_documents` tool to scope searches to a specific set."
);

pub const AGENTS_URI: &str = "resource://agents";
pub const AGENTS_NAME: &str = "agents";
pub const AGENTS_DESCRIPTION: &str = concat!(
    "Enumerate the Lumen agents accessible to the current user. Use a ",
    "returned `name` value with the `agent` filter of the ",
    "`search_indexed_documents` tool to run a search with that agent's ",
    "knowledge scope and model."
);

/// Minimal document-set shape surfaced to MCP clients, projected from the
/// backend's `DocumentSetSummary` so MCP stays decoupled from admin-only fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentSetEntry {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Minimal agent (persona) shape, projected from `MinimalPersonaSnapshot`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentEntry {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Shape of `GET /manage/indexed-sources`.
#[derive(Debug, Deserialize)]
struct IndexedSourcesPayload {
    #[serde(default)]
    sources: Vec<serde_json::Value>,
}

/// Fetch the indexed document sources for the caller.
///
/// Values are stringified rather than required to be strings, matching the
/// Python `[str(source) for source in sources]`.
pub async fn get_indexed_sources(
    client: &ApiClient,
    token: &AccessToken,
) -> Result<Vec<String>, UpstreamError> {
    let payload: IndexedSourcesPayload = client
        .get_json("/manage/indexed-sources", &token.token)
        .await?;

    Ok(payload
        .sources
        .into_iter()
        .map(|source| match source {
            serde_json::Value::String(text) => text,
            other => other.to_string(),
        })
        .collect())
}

/// Fetch the document sets the caller can filter by.
pub async fn get_accessible_document_sets(
    client: &ApiClient,
    token: &AccessToken,
) -> Result<Vec<DocumentSetEntry>, UpstreamError> {
    client.get_json("/manage/document-set", &token.token).await
}

/// Fetch the agents the caller can scope a search to.
pub async fn get_accessible_agents(
    client: &ApiClient,
    token: &AccessToken,
) -> Result<Vec<AgentEntry>, UpstreamError> {
    client.get_json("/persona", &token.token).await
}

/// Body of `resource://indexed_sources`: a sorted JSON array of source names.
pub async fn indexed_sources_body(
    client: &ApiClient,
    token: &AccessToken,
) -> Result<String, UpstreamError> {
    let mut sources = get_indexed_sources(client, token).await?;
    sources.sort();

    tracing::info!(
        entries = sources.len(),
        "Lumen MCP Server: indexed_sources resource returning entries"
    );
    Ok(python_json::to_string(&sources).unwrap_or_else(|_| "[]".to_string()))
}

/// Body of `resource://document_sets`: entries sorted by name.
pub async fn document_sets_body(
    client: &ApiClient,
    token: &AccessToken,
) -> Result<String, UpstreamError> {
    let mut sets = get_accessible_document_sets(client, token).await?;
    sets.sort_by(|left, right| left.name.cmp(&right.name));

    tracing::info!(
        entries = sets.len(),
        "Lumen MCP Server: document_sets resource returning entries"
    );
    Ok(python_json::to_string(&sets).unwrap_or_else(|_| "[]".to_string()))
}

/// Body of `resource://agents`: entries sorted by name.
pub async fn agents_body(client: &ApiClient, token: &AccessToken) -> Result<String, UpstreamError> {
    let mut agents = get_accessible_agents(client, token).await?;
    agents.sort_by(|left, right| left.name.cmp(&right.name));

    tracing::info!(
        entries = agents.len(),
        "Lumen MCP Server: agents resource returning entries"
    );
    Ok(python_json::to_string(&agents).unwrap_or_else(|_| "[]".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_set_entry_keeps_only_name_and_description() {
        // The API returns admin-only fields too; they must not reach clients.
        let raw = r#"{"id":7,"name":"Wiki","description":"Docs","cc_pair_summaries":[]}"#;
        let entry: DocumentSetEntry = serde_json::from_str(raw).unwrap();
        assert_eq!(entry.name, "Wiki");
        assert_eq!(entry.description.as_deref(), Some("Docs"));
        assert_eq!(
            python_json::to_string(&entry).unwrap(),
            r#"{"name": "Wiki", "description": "Docs"}"#
        );
    }

    #[test]
    fn agent_entry_keeps_only_id_name_and_description() {
        let raw = r#"{"id":5,"name":"Support","description":null,"tools":[],"icon_shape":3}"#;
        let entry: AgentEntry = serde_json::from_str(raw).unwrap();
        assert_eq!(entry.id, 5);
        assert_eq!(
            python_json::to_string(&entry).unwrap(),
            r#"{"id": 5, "name": "Support", "description": null}"#
        );
    }

    #[test]
    fn a_missing_description_deserializes_to_none() {
        let entry: DocumentSetEntry = serde_json::from_str(r#"{"name":"Only"}"#).unwrap();
        assert_eq!(entry.description, None);
    }

    #[test]
    fn indexed_sources_payload_defaults_to_empty() {
        let payload: IndexedSourcesPayload = serde_json::from_str("{}").unwrap();
        assert!(payload.sources.is_empty());
    }

    #[test]
    fn resource_descriptions_match_the_python_decorators() {
        assert_eq!(
            INDEXED_SOURCES_DESCRIPTION,
            "Enumerate the user's document sources that are currently indexed in Lumen.\
This can be used to discover filters for the `search_indexed_documents` tool."
        );
        assert!(DOCUMENT_SETS_DESCRIPTION.starts_with("Enumerate the Document Sets"));
        assert!(AGENTS_DESCRIPTION.starts_with("Enumerate the Lumen agents"));
    }
}
