//! Log setup.
//!
//! `LUMEN_MCP_LOG` (or `RUST_LOG`) sets the filter; `LOG_JSON=true` switches to
//! JSON lines for cluster log collectors.

use tracing_subscriber::{fmt, EnvFilter};

pub fn init() {
    let filter = EnvFilter::try_from_env("LUMEN_MCP_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let json = std::env::var("LOG_JSON")
        .map(|value| value.to_lowercase() == "true")
        .unwrap_or(false);

    let builder = fmt().with_env_filter(filter).with_target(true);
    if json {
        builder.json().init();
    } else {
        builder.init();
    }
}
