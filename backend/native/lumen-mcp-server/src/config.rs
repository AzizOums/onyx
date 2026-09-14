//! Server configuration, read from the environment.
//!
//! Every name and default here mirrors `backend/lumen/configs/app_configs.py`,
//! so the Rust and Python servers answer to the same environment.

use std::time::Duration;

/// The API server listens on a fixed port; `APP_PORT` in app_configs.py is a
/// constant, not an environment variable.
const APP_PORT: u16 = 8080;

const DEFAULT_API_REQUEST_TIMEOUT_SECONDS: u64 = 300;
const DEFAULT_MCP_SERVER_PORT: u16 = 8090;
/// Matches the connect timeout the Python `_post_model` helper passes to httpx.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct Config {
    /// When false the process logs and exits without binding a port, like the
    /// Python entry point.
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    /// Empty means CORS stays off, as in the Python server.
    pub cors_origins: Vec<String>,
    pub api_request_timeout: Duration,
    /// Base URL of the Lumen API, prefix included. Every upstream call joins a
    /// path onto this.
    pub api_base_url: String,
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|value| value.to_lowercase() == "true")
        .unwrap_or(false)
}

fn env_or(name: &str, fallback: &str) -> String {
    match std::env::var(name) {
        Ok(value) => value,
        Err(_) => fallback.to_string(),
    }
}

/// Port of `build_api_server_url_for_http_requests(respect_env_override_if_set=True)`.
///
/// Dev mode wins over the override, which wins over protocol/host. The API
/// prefix is appended last in every case.
pub fn build_api_base_url() -> String {
    let mut url = if env_flag("DEV_MODE") {
        format!("http://127.0.0.1:{APP_PORT}")
    } else {
        match std::env::var("API_SERVER_URL_OVERRIDE_FOR_HTTP_REQUESTS") {
            Ok(override_url) if !override_url.is_empty() => {
                override_url.trim_end_matches('/').to_string()
            }
            _ => {
                let protocol = env_or("API_SERVER_PROTOCOL", "http");
                let host = env_or("API_SERVER_HOST", "127.0.0.1");
                format!("{protocol}://{host}:{APP_PORT}")
            }
        }
    };

    let prefix = env_or("API_PREFIX", "");
    let prefix = prefix.trim_matches('/');
    if !prefix.is_empty() {
        url.push('/');
        url.push_str(prefix);
    }
    url
}

/// Port of the `MCP_SERVER_API_REQUEST_TIMEOUT_SECONDS` parsing: a value that is
/// absent, empty, unparseable or non-positive falls back to 300 seconds.
fn api_request_timeout() -> Duration {
    let seconds = std::env::var("MCP_SERVER_API_REQUEST_TIMEOUT_SECONDS")
        .ok()
        .filter(|raw| !raw.is_empty())
        .and_then(|raw| raw.parse::<i64>().ok())
        .filter(|seconds| *seconds > 0)
        .map_or(DEFAULT_API_REQUEST_TIMEOUT_SECONDS, |seconds| {
            seconds as u64
        });
    Duration::from_secs(seconds)
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            enabled: env_flag("MCP_SERVER_ENABLED"),
            // Binding every interface is deliberate: the server runs in a container.
            host: env_or("MCP_SERVER_HOST", "0.0.0.0"),
            port: std::env::var("MCP_SERVER_PORT")
                .ok()
                .filter(|raw| !raw.is_empty())
                .and_then(|raw| raw.parse().ok())
                .unwrap_or(DEFAULT_MCP_SERVER_PORT),
            cors_origins: env_or("MCP_SERVER_CORS_ORIGINS", "")
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_string)
                .collect(),
            api_request_timeout: api_request_timeout(),
            api_base_url: build_api_base_url(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Environment mutation is process-wide, so these tests take a lock.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const URL_VARS: [&str; 5] = [
        "DEV_MODE",
        "API_SERVER_URL_OVERRIDE_FOR_HTTP_REQUESTS",
        "API_SERVER_PROTOCOL",
        "API_SERVER_HOST",
        "API_PREFIX",
    ];

    fn with_env<T>(pairs: &[(&str, &str)], body: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        for name in URL_VARS {
            std::env::remove_var(name);
        }
        std::env::remove_var("MCP_SERVER_API_REQUEST_TIMEOUT_SECONDS");
        for (name, value) in pairs {
            std::env::set_var(name, value);
        }
        let out = body();
        for name in URL_VARS {
            std::env::remove_var(name);
        }
        std::env::remove_var("MCP_SERVER_API_REQUEST_TIMEOUT_SECONDS");
        out
    }

    #[test]
    fn default_api_url_uses_protocol_host_and_app_port() {
        assert_eq!(with_env(&[], build_api_base_url), "http://127.0.0.1:8080");
    }

    #[test]
    fn dev_mode_wins_over_the_override() {
        let url = with_env(
            &[
                ("DEV_MODE", "true"),
                ("API_SERVER_URL_OVERRIDE_FOR_HTTP_REQUESTS", "https://cloud"),
            ],
            build_api_base_url,
        );
        assert_eq!(url, "http://127.0.0.1:8080");
    }

    #[test]
    fn override_wins_over_protocol_and_host_and_drops_trailing_slash() {
        let url = with_env(
            &[
                (
                    "API_SERVER_URL_OVERRIDE_FOR_HTTP_REQUESTS",
                    "https://cloud/",
                ),
                ("API_SERVER_HOST", "ignored"),
            ],
            build_api_base_url,
        );
        assert_eq!(url, "https://cloud");
    }

    #[test]
    fn protocol_and_host_are_read_from_the_environment() {
        let url = with_env(
            &[
                ("API_SERVER_PROTOCOL", "https"),
                ("API_SERVER_HOST", "api.internal"),
            ],
            build_api_base_url,
        );
        assert_eq!(url, "https://api.internal:8080");
    }

    #[test]
    fn api_prefix_is_appended_with_one_slash() {
        for prefix in ["api", "/api", "/api/"] {
            let url = with_env(&[("API_PREFIX", prefix)], build_api_base_url);
            assert_eq!(url, "http://127.0.0.1:8080/api", "prefix {prefix:?}");
        }
    }

    #[test]
    fn timeout_falls_back_to_300_when_unusable() {
        for raw in ["", "0", "-5", "not-a-number"] {
            let timeout = with_env(&[("MCP_SERVER_API_REQUEST_TIMEOUT_SECONDS", raw)], || {
                api_request_timeout()
            });
            assert_eq!(timeout, Duration::from_secs(300), "raw {raw:?}");
        }
    }

    #[test]
    fn timeout_reads_a_positive_value() {
        let timeout = with_env(&[("MCP_SERVER_API_REQUEST_TIMEOUT_SECONDS", "45")], || {
            api_request_timeout()
        });
        assert_eq!(timeout, Duration::from_secs(45));
    }
}
