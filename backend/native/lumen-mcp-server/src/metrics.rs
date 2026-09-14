//! Prometheus metrics.
//!
//! Names, labels and bucket boundaries match
//! `backend/lumen/server/metrics/mcp_server.py` exactly, so existing dashboards
//! and alerts keep working after the cutover.

use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry, TextEncoder};
use std::sync::LazyLock;

/// Outcome of a token verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthResult {
    Success,
    /// The API server answered, and said no.
    Rejected,
    /// The API server could not be reached.
    Error,
}

impl AuthResult {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Rejected => "rejected",
            Self::Error => "error",
        }
    }
}

/// Outcome of a tool call. Mirrors `MCPToolCallStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
    Success,
    AuthError,
    Error,
}

impl ToolCallStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::AuthError => "auth_error",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolName {
    SearchIndexedDocuments,
    SearchWeb,
    OpenUrls,
}

impl ToolName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SearchIndexedDocuments => "search_indexed_documents",
            Self::SearchWeb => "search_web",
            Self::OpenUrls => "open_urls",
        }
    }
}

/// Label used when a requested source type is not a known `DocumentSource`.
pub const UNKNOWN_SOURCE_LABEL: &str = "unknown";

pub struct Metrics {
    pub registry: Registry,
    auth_total: IntCounterVec,
    tool_latency: HistogramVec,
    tool_total: IntCounterVec,
    search_results: HistogramVec,
    search_sources: IntCounterVec,
}

impl Metrics {
    fn new() -> Self {
        let registry = Registry::new();

        let auth_total = IntCounterVec::new(
            Opts::new(
                "lumen_mcp_server_auth_total",
                "MCP server token verification outcomes",
            ),
            &["result"],
        )
        .expect("auth counter is well formed");

        let tool_latency = HistogramVec::new(
            HistogramOpts::new(
                "lumen_mcp_server_tool_latency_seconds",
                "MCP server tool execution latency",
            )
            .buckets(vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0]),
            &["tool"],
        )
        .expect("latency histogram is well formed");

        let tool_total = IntCounterVec::new(
            Opts::new("lumen_mcp_server_tool_calls_total", "MCP server tool calls"),
            &["tool", "status"],
        )
        .expect("tool counter is well formed");

        let search_results = HistogramVec::new(
            HistogramOpts::new(
                "lumen_mcp_server_search_results",
                "Results returned by MCP server search tools",
            )
            .buckets(vec![0.0, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0]),
            &["tool"],
        )
        .expect("results histogram is well formed");

        let search_sources = IntCounterVec::new(
            Opts::new(
                "lumen_mcp_server_search_by_source_total",
                "MCP server document searches by requested source type",
            ),
            &["source_type"],
        )
        .expect("source counter is well formed");

        for collector in [
            Box::new(auth_total.clone()) as Box<dyn prometheus::core::Collector>,
            Box::new(tool_latency.clone()),
            Box::new(tool_total.clone()),
            Box::new(search_results.clone()),
            Box::new(search_sources.clone()),
        ] {
            registry
                .register(collector)
                .expect("each collector is registered once");
        }

        Self {
            registry,
            auth_total,
            tool_latency,
            tool_total,
            search_results,
            search_sources,
        }
    }

    pub fn record_auth(&self, result: AuthResult) {
        self.auth_total.with_label_values(&[result.as_str()]).inc();
    }

    pub fn record_tool_outcome(&self, tool: ToolName, elapsed_secs: f64, status: ToolCallStatus) {
        self.tool_latency
            .with_label_values(&[tool.as_str()])
            .observe(elapsed_secs);
        self.tool_total
            .with_label_values(&[tool.as_str(), status.as_str()])
            .inc();
    }

    pub fn record_search_results(&self, tool: ToolName, count: usize) {
        self.search_results
            .with_label_values(&[tool.as_str()])
            .observe(count as f64);
    }

    pub fn record_search_source(&self, source_type: &str) {
        self.search_sources.with_label_values(&[source_type]).inc();
    }

    /// Render the registry in the Prometheus text exposition format.
    pub fn encode(&self) -> String {
        TextEncoder::new()
            .encode_to_string(&self.registry.gather())
            .unwrap_or_default()
    }
}

/// Process-wide metrics, like the Python module-level collectors.
pub static METRICS: LazyLock<Metrics> = LazyLock::new(Metrics::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_values_match_the_python_enums() {
        assert_eq!(AuthResult::Success.as_str(), "success");
        assert_eq!(AuthResult::Rejected.as_str(), "rejected");
        assert_eq!(AuthResult::Error.as_str(), "error");
        assert_eq!(ToolCallStatus::AuthError.as_str(), "auth_error");
        assert_eq!(
            ToolName::SearchIndexedDocuments.as_str(),
            "search_indexed_documents"
        );
        assert_eq!(ToolName::SearchWeb.as_str(), "search_web");
        assert_eq!(ToolName::OpenUrls.as_str(), "open_urls");
        assert_eq!(ToolCallStatus::Success.as_str(), "success");
        assert_eq!(ToolCallStatus::Error.as_str(), "error");
    }

    #[test]
    fn encoding_exposes_every_series_name() {
        let metrics = Metrics::new();
        metrics.record_auth(AuthResult::Success);
        metrics.record_tool_outcome(ToolName::SearchWeb, 0.2, ToolCallStatus::Success);
        metrics.record_search_results(ToolName::SearchWeb, 3);
        metrics.record_search_source("jira");

        let text = metrics.encode();
        for name in [
            "lumen_mcp_server_auth_total",
            "lumen_mcp_server_tool_latency_seconds",
            "lumen_mcp_server_tool_calls_total",
            "lumen_mcp_server_search_results",
            "lumen_mcp_server_search_by_source_total",
        ] {
            assert!(text.contains(name), "missing series {name}");
        }
        assert!(text.contains(r#"result="success""#));
        assert!(text.contains(r#"source_type="jira""#));
    }
}
