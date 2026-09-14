//! The MCP request handler.
//!
//! Wires the ported tools and resources onto rmcp's `ServerHandler`. The bearer
//! token is verified by the HTTP middleware before a request reaches here, so
//! this layer only reads the verified token back out of the request extensions.

use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListResourcesResult,
    ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse,
    ReadResourceResult, Resource, ResourceContents, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer};
use serde_json::Value;

use crate::auth::AccessToken;
use crate::resources as res;
use crate::tools;
use crate::upstream::ApiClient;

/// Name and version reported to clients. Both match the Python `FastMCP(...)`
/// constructor, so a client cannot tell the two servers apart.
pub const SERVER_NAME: &str = "Lumen MCP Server";
pub const SERVER_VERSION: &str = "1.0.0";

#[derive(Clone)]
pub struct LumenMcpHandler {
    client: ApiClient,
}

impl LumenMcpHandler {
    pub fn new(client: ApiClient) -> Self {
        Self { client }
    }

    /// Read the token the HTTP middleware verified and attached.
    ///
    /// Reaching a handler without one means the middleware was bypassed, which
    /// is a wiring bug rather than a client error.
    fn access_token(context: &RequestContext<RoleServer>) -> Result<AccessToken, McpError> {
        context
            .extensions
            .get::<http::request::Parts>()
            .and_then(|parts| parts.extensions.get::<AccessToken>())
            .cloned()
            .ok_or_else(|| {
                McpError::internal_error(
                    "MCP Server requires an Lumen access token to authenticate your request",
                    None,
                )
            })
    }
}

/// Every tool returns a JSON object. `structured` carries it as structured
/// content and mirrors it as text, which is what MCP clients expect.
fn json_tool_result(value: Value) -> CallToolResult {
    CallToolResult::structured(value)
}

fn parse_args<T: serde::de::DeserializeOwned>(
    request: &CallToolRequestParams,
) -> Result<T, McpError> {
    let arguments = request.arguments.clone().unwrap_or_default();
    serde_json::from_value(Value::Object(arguments))
        .map_err(|err| McpError::invalid_params(err.to_string(), None))
}

impl ServerHandler for LumenMcpHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new(SERVER_NAME, SERVER_VERSION))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: vec![
                Tool::new(
                    "search_indexed_documents",
                    tools::SEARCH_INDEXED_DOCUMENTS_DESCRIPTION,
                    Arc::new(tools::search_indexed_documents_schema()),
                ),
                Tool::new(
                    "search_web",
                    tools::SEARCH_WEB_DESCRIPTION,
                    Arc::new(tools::search_web_schema()),
                ),
                Tool::new(
                    "open_urls",
                    tools::OPEN_URLS_DESCRIPTION,
                    Arc::new(tools::open_urls_schema()),
                ),
            ],
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let token = Self::access_token(&context)?;

        let value = match request.name.as_ref() {
            "search_indexed_documents" => {
                tools::search_indexed_documents(&self.client, &token, parse_args(&request)?).await
            }
            "search_web" => tools::search_web(&self.client, &token, parse_args(&request)?).await,
            "open_urls" => tools::open_urls(&self.client, &token, parse_args(&request)?).await,
            unknown => {
                return Err(McpError::invalid_params(
                    format!("Unknown tool: {unknown}"),
                    None,
                ))
            }
        };

        Ok(json_tool_result(value).into())
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![
                Resource::new(res::INDEXED_SOURCES_URI, res::INDEXED_SOURCES_NAME)
                    .with_description(res::INDEXED_SOURCES_DESCRIPTION)
                    .with_mime_type(res::MIME_TYPE),
                Resource::new(res::DOCUMENT_SETS_URI, res::DOCUMENT_SETS_NAME)
                    .with_description(res::DOCUMENT_SETS_DESCRIPTION)
                    .with_mime_type(res::MIME_TYPE),
                Resource::new(res::AGENTS_URI, res::AGENTS_NAME)
                    .with_description(res::AGENTS_DESCRIPTION)
                    .with_mime_type(res::MIME_TYPE),
            ],
            ..Default::default()
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let token = Self::access_token(&context)?;

        let body = match request.uri.as_str() {
            res::INDEXED_SOURCES_URI => res::indexed_sources_body(&self.client, &token).await,
            res::DOCUMENT_SETS_URI => res::document_sets_body(&self.client, &token).await,
            res::AGENTS_URI => res::agents_body(&self.client, &token).await,
            unknown => {
                return Err(McpError::resource_not_found(
                    format!("Unknown resource: {unknown}"),
                    None,
                ))
            }
        };

        // A resource has no error envelope to fall back on, so an upstream
        // failure surfaces as a protocol error — the Python resources raise too.
        let text = body.map_err(|err| McpError::internal_error(err.to_string(), None))?;

        Ok(
            ReadResourceResult::new(vec![ResourceContents::TextResourceContents {
                uri: request.uri,
                mime_type: Some(res::MIME_TYPE.to_string()),
                text,
                meta: None,
            }])
            .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_identifies_itself_like_the_python_server() {
        let client = ApiClient::with_base_url("http://api", std::time::Duration::from_secs(1))
            .expect("client builds");
        let info = LumenMcpHandler::new(client).get_info();
        assert_eq!(info.server_info.name, "Lumen MCP Server");
        assert_eq!(info.server_info.version, "1.0.0");
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.resources.is_some());
    }

    #[test]
    fn a_tool_result_carries_both_text_and_structured_content() {
        let result = json_tool_result(serde_json::json!({"results": []}));
        assert_eq!(
            result.structured_content,
            Some(serde_json::json!({"results": []}))
        );
        assert_eq!(result.content.len(), 1);
    }
}
