//! Lumen MCP server.
//!
//! A port of `backend/lumen/mcp_server/` (Python, FastMCP). The server owns no
//! data: it authenticates bearer tokens against the Lumen API and forwards every
//! tool and resource call to that same API, so it needs no database access.

pub mod auth;
pub mod config;
pub mod http;
pub mod logging;
pub mod metrics;
pub mod resources;
pub mod server;
pub mod time_cutoff;
pub mod tools;
pub mod upstream;
