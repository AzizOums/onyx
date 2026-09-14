//! Lumen MCP server entry point.
//!
//! Port of `backend/lumen/mcp_server_main.py`: read the environment, exit
//! quietly when the server is disabled, then serve until asked to stop.

use std::process::ExitCode;

use lumen_mcp_server::{config::Config, http, logging, upstream::ApiClient};

fn main() -> ExitCode {
    logging::init();

    let config = Config::from_env();

    if !config.enabled {
        tracing::info!("MCP server is disabled (MCP_SERVER_ENABLED=false)");
        return ExitCode::SUCCESS;
    }

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(err) => {
            tracing::error!(error = %err, "Failed to start the async runtime");
            return ExitCode::FAILURE;
        }
    };

    runtime.block_on(async move {
        let client = match ApiClient::new(&config) {
            Ok(client) => client,
            Err(err) => {
                tracing::error!(error = %err, "Failed to build the Lumen API client");
                return ExitCode::FAILURE;
            }
        };

        tracing::info!(api_base_url = client.base_url(), "Lumen API base URL");

        match http::serve(&config, client).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                tracing::error!(error = %err, "MCP server exited with an error");
                ExitCode::FAILURE
            }
        }
    })
}
