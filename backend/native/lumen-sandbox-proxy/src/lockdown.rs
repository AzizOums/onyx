//! The internal-destination egress lockdown.
//!
//! Port of `destination_is_blocked` and its helpers in
//! `backend/lumen/sandbox_proxy/addons/gate.py`.
//!
//! A sandbox can only reach the network through this proxy, so the proxy is the
//! single layer that can stop it relaying to internal services — databases,
//! caches, search, cloud metadata endpoints. That destination is invisible at
//! the sandbox's own egress, which sees only "TCP to proxy:8080".
//!
//! The rule is "not globally routable", not a hostname deny-list, so it also
//! covers the internal services nobody enumerated. The one legitimate internal
//! exception is the api-server the sandbox calls through the proxy, matched by
//! host *and* port so a co-located database on the same hostname stays denied.

use std::net::IpAddr;
use std::sync::OnceLock;

use serde::Deserialize;

const RANGES_JSON: &str = include_str!("../data/ip_special_ranges.json");

/// One CIDR block, as `data/ip_special_ranges.json` carries it.
#[derive(Debug, Clone, Copy)]
struct Cidr {
    network: IpAddr,
    prefix_len: u8,
}

impl Cidr {
    fn parse(text: &str) -> Option<Self> {
        let (address, prefix) = text.split_once('/')?;
        Some(Self {
            network: address.parse().ok()?,
            prefix_len: prefix.parse().ok()?,
        })
    }

    fn contains(&self, address: IpAddr) -> bool {
        match (self.network, address) {
            (IpAddr::V4(network), IpAddr::V4(address)) => {
                masked(&network.octets(), &address.octets(), self.prefix_len)
            }
            (IpAddr::V6(network), IpAddr::V6(address)) => {
                masked(&network.octets(), &address.octets(), self.prefix_len)
            }
            _ => false,
        }
    }
}

/// Whether two addresses agree on their first `prefix_len` bits.
fn masked(network: &[u8], address: &[u8], prefix_len: u8) -> bool {
    let full_bytes = (prefix_len / 8) as usize;
    let spare_bits = prefix_len % 8;
    if network[..full_bytes] != address[..full_bytes] {
        return false;
    }
    if spare_bits == 0 {
        return true;
    }
    let mask = 0xFFu8 << (8 - spare_bits);
    network[full_bytes] & mask == address[full_bytes] & mask
}

#[derive(Debug, Deserialize)]
struct RawFamily {
    private: Vec<String>,
    private_exceptions: Vec<String>,
    not_global: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawRanges {
    v4: RawFamily,
    v6: RawFamily,
}

struct Family {
    private: Vec<Cidr>,
    private_exceptions: Vec<Cidr>,
    not_global: Vec<Cidr>,
}

impl Family {
    fn parse(raw: &RawFamily) -> Self {
        let parse_all = |blocks: &[String]| {
            blocks
                .iter()
                .map(|block| {
                    Cidr::parse(block)
                        .unwrap_or_else(|| panic!("unparsable CIDR in the export: {block}"))
                })
                .collect()
        };
        Self {
            private: parse_all(&raw.private),
            private_exceptions: parse_all(&raw.private_exceptions),
            not_global: parse_all(&raw.not_global),
        }
    }

    fn is_private(&self, address: IpAddr) -> bool {
        self.private.iter().any(|block| block.contains(address))
            && !self
                .private_exceptions
                .iter()
                .any(|block| block.contains(address))
    }

    fn is_global(&self, address: IpAddr) -> bool {
        !self.not_global.iter().any(|block| block.contains(address)) && !self.is_private(address)
    }
}

struct Ranges {
    v4: Family,
    v6: Family,
}

fn ranges() -> &'static Ranges {
    static RANGES: OnceLock<Ranges> = OnceLock::new();
    RANGES.get_or_init(|| {
        let raw: RawRanges = serde_json::from_str(RANGES_JSON)
            .expect("data/ip_special_ranges.json is generated; regenerate it");
        Ranges {
            v4: Family::parse(&raw.v4),
            v6: Family::parse(&raw.v6),
        }
    })
}

/// Whether an address is not globally routable, and therefore internal.
///
/// Covers far more than RFC 1918: carrier-grade NAT (`100.64.0.0/10`, which EKS
/// pods use under custom networking), loopback, link-local — which includes the
/// cloud metadata endpoint — IPv6 unique-local, link-local and loopback, and the
/// reserved ranges. An IPv4-mapped IPv6 address (`::ffff:10.0.0.1`) is judged by
/// its embedded IPv4, so it cannot smuggle an internal v4 address past the
/// check.
pub fn is_internal(address: IpAddr) -> bool {
    !is_global(address)
}

fn is_global(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(_) => ranges().v4.is_global(address),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(mapped) => ranges().v4.is_global(IpAddr::V4(mapped)),
            None => ranges().v6.is_global(address),
        },
    }
}

/// The single allowed internal destination: the api-server the sandbox calls
/// through the proxy.
///
/// Matched by host *and* port, so it works when the api-server is an in-cluster
/// name resolving to an internal address, while every other port on that host —
/// a co-located Redis or Postgres — stays denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiServer {
    host: String,
    port: u16,
}

impl ApiServer {
    /// Parse the configured server URL. `None` when it is unset or has no host,
    /// which leaves no internal destination allowed at all.
    pub fn from_url(url: &str) -> Option<Self> {
        let url = url.trim();
        if url.is_empty() {
            return None;
        }
        let (scheme, rest) = url.split_once("://")?;
        // Strip userinfo, then the path, query and fragment.
        let authority = rest
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .rsplit('@')
            .next()
            .unwrap_or("");
        let (host, port) = split_host_port(authority);
        let host = host.trim().to_ascii_lowercase();
        if host.is_empty() {
            return None;
        }
        let port = port.unwrap_or(if scheme.eq_ignore_ascii_case("https") {
            443
        } else {
            80
        });
        Some(Self { host, port })
    }

    fn matches(&self, host: &str, port: u16) -> bool {
        self.host == host && self.port == port
    }
}

/// Split an authority into host and optional port, honouring the brackets an
/// IPv6 literal is written with.
fn split_host_port(authority: &str) -> (&str, Option<u16>) {
    if let Some(rest) = authority.strip_prefix('[') {
        let Some((host, after)) = rest.split_once(']') else {
            return (authority, None);
        };
        let port = after.strip_prefix(':').and_then(|p| p.parse().ok());
        return (host, port);
    }
    match authority.rsplit_once(':') {
        Some((host, port)) => (host, port.parse().ok()),
        None => (authority, None),
    }
}

/// Resolves a hostname to the addresses a connection would reach.
///
/// A trait so the lockdown's decision can be tested without a resolver, and so
/// the caller owns whether resolution blocks.
pub trait Resolver {
    /// Every address `host:port` resolves to, or `Err` when resolution failed.
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, String>;
}

/// The default resolver: the system one, as `socket.getaddrinfo` uses.
pub struct SystemResolver;

impl Resolver for SystemResolver {
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, String> {
        use std::net::ToSocketAddrs;
        (host, port)
            .to_socket_addrs()
            .map(|addresses| addresses.map(|address| address.ip()).collect())
            .map_err(|error| error.to_string())
    }
}

/// Whether the sandbox must not be relayed to `host:port`.
///
/// Denied: anything that is, or resolves to, an internal address. Allowed: the
/// api-server and any public address.
///
/// Fails closed. A resolution failure denies — a transient resolver error must
/// not become an opening to an internal service. A name resolving to a mix of
/// public and internal addresses denies too: otherwise an attacker could steer
/// the connection to the internal one.
pub fn destination_is_blocked(
    host: &str,
    port: u16,
    api_server: Option<&ApiServer>,
    resolver: &dyn Resolver,
) -> bool {
    let host = host.trim().to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    if api_server.is_some_and(|server| server.matches(&host, port)) {
        return false;
    }

    // A literal-IP destination is checked directly — no DNS to be steered.
    // Written unbracketed, as the transport layer reports it; a bracketed form
    // is not a literal here and falls through to resolution, which fails and
    // therefore denies.
    if let Ok(address) = host.parse::<IpAddr>() {
        return is_internal(address);
    }

    match resolver.resolve(&host, port) {
        Err(error) => {
            tracing::warn!(host = %host, error = %error, "egress_destination_resolution_failed");
            true
        }
        Ok(addresses) => addresses.iter().copied().any(is_internal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FixedResolver(HashMap<String, Result<Vec<IpAddr>, String>>);

    impl Resolver for FixedResolver {
        fn resolve(&self, host: &str, _port: u16) -> Result<Vec<IpAddr>, String> {
            self.0
                .get(host)
                .cloned()
                .unwrap_or_else(|| Err("no such host".into()))
        }
    }

    fn resolver(entries: &[(&str, Result<Vec<&str>, &str>)]) -> FixedResolver {
        FixedResolver(
            entries
                .iter()
                .map(|(host, answer)| {
                    let value = match answer {
                        Ok(addresses) => Ok(addresses.iter().map(|a| a.parse().unwrap()).collect()),
                        Err(error) => Err((*error).to_string()),
                    };
                    ((*host).to_string(), value)
                })
                .collect(),
        )
    }

    fn ip(text: &str) -> IpAddr {
        text.parse().expect("a literal address")
    }

    #[test]
    fn public_addresses_are_not_internal() {
        for address in ["8.8.8.8", "1.1.1.1", "93.184.216.34", "2606:4700::1111"] {
            assert!(!is_internal(ip(address)), "{address} read as internal");
        }
    }

    #[test]
    fn the_ranges_a_deny_list_would_have_missed_are_internal() {
        for address in [
            "10.0.0.1",        // RFC 1918
            "172.16.0.1",      // RFC 1918
            "192.168.1.1",     // RFC 1918
            "127.0.0.1",       // loopback
            "0.0.0.0",         // this network
            "100.64.0.1",      // carrier-grade NAT: EKS pods under custom networking
            "169.254.169.254", // the cloud metadata endpoint
            "198.18.0.1",      // benchmarking
            "240.0.0.1",       // reserved
            "255.255.255.255", // broadcast
            "::1",             // IPv6 loopback
            "fc00::1",         // IPv6 unique-local
            "fe80::1",         // IPv6 link-local
        ] {
            assert!(is_internal(ip(address)), "{address} read as public");
        }
    }

    #[test]
    fn an_ipv4_mapped_address_cannot_smuggle_an_internal_address() {
        // The whole point: `::ffff:10.0.0.1` is judged by its embedded IPv4.
        assert!(is_internal(ip("::ffff:10.0.0.1")));
        assert!(is_internal(ip("::ffff:169.254.169.254")));
        assert!(!is_internal(ip("::ffff:8.8.8.8")));
    }

    #[test]
    fn the_api_server_is_the_one_allowed_internal_destination() {
        let api = ApiServer::from_url("http://lumen-api-server:8080").unwrap();
        let dns = resolver(&[("lumen-api-server", Ok(vec!["10.0.1.5"]))]);

        assert!(!destination_is_blocked(
            "lumen-api-server",
            8080,
            Some(&api),
            &dns
        ));
        // Every other port on the same host stays denied: a co-located Redis or
        // Postgres is reachable at exactly that hostname.
        assert!(destination_is_blocked(
            "lumen-api-server",
            6379,
            Some(&api),
            &dns
        ));
    }

    #[test]
    fn a_host_is_matched_case_insensitively() {
        let api = ApiServer::from_url("https://API.Example.COM").unwrap();
        let dns = resolver(&[]);
        assert!(!destination_is_blocked(
            "api.example.com",
            443,
            Some(&api),
            &dns
        ));
        assert!(!destination_is_blocked(
            "API.EXAMPLE.COM",
            443,
            Some(&api),
            &dns
        ));
    }

    #[test]
    fn a_resolution_failure_denies() {
        // Fail closed: a transient resolver error must not become an opening.
        let dns = resolver(&[("wobbly.internal", Err("SERVFAIL"))]);
        assert!(destination_is_blocked("wobbly.internal", 443, None, &dns));
    }

    #[test]
    fn a_name_resolving_to_a_mix_denies() {
        // Otherwise an attacker steers the connection to the internal answer.
        let dns = resolver(&[("split.example.com", Ok(vec!["93.184.216.34", "10.0.0.7"]))]);
        assert!(destination_is_blocked("split.example.com", 443, None, &dns));
    }

    #[test]
    fn a_name_resolving_only_to_public_addresses_is_allowed() {
        let dns = resolver(&[("example.com", Ok(vec!["93.184.216.34", "2606:4700::1111"]))]);
        assert!(!destination_is_blocked("example.com", 443, None, &dns));
    }

    #[test]
    fn a_literal_destination_is_judged_without_dns() {
        // No resolver entry, so a lookup would deny; the literal must decide.
        let dns = resolver(&[]);
        assert!(!destination_is_blocked("8.8.8.8", 443, None, &dns));
        assert!(destination_is_blocked("10.0.0.1", 443, None, &dns));
        assert!(destination_is_blocked("::1", 443, None, &dns));
        assert!(!destination_is_blocked("2606:4700::1111", 443, None, &dns));
        // A bracketed form is not what the transport layer reports, so it is
        // not treated as a literal: it goes to the resolver, which fails, which
        // denies. Parsing it here would ALLOW a public literal Python blocks.
        assert!(destination_is_blocked("[2606:4700::1111]", 443, None, &dns));
    }

    #[test]
    fn an_empty_host_is_not_blocked() {
        let dns = resolver(&[]);
        assert!(!destination_is_blocked("", 443, None, &dns));
        assert!(!destination_is_blocked("   ", 443, None, &dns));
    }

    #[test]
    fn with_no_api_server_configured_no_internal_destination_is_allowed() {
        let dns = resolver(&[("lumen-api-server", Ok(vec!["10.0.1.5"]))]);
        assert!(destination_is_blocked("lumen-api-server", 8080, None, &dns));
    }

    #[test]
    fn a_server_url_gets_its_schemes_default_port() {
        assert_eq!(
            ApiServer::from_url("https://api.example.com").unwrap().port,
            443
        );
        assert_eq!(
            ApiServer::from_url("http://api.example.com").unwrap().port,
            80
        );
        assert_eq!(
            ApiServer::from_url("https://api.example.com:8443")
                .unwrap()
                .port,
            8443
        );
        assert_eq!(
            ApiServer::from_url("http://api.example.com/some/path")
                .unwrap()
                .host,
            "api.example.com"
        );
    }

    #[test]
    fn an_unusable_server_url_configures_no_exception() {
        assert!(ApiServer::from_url("").is_none());
        assert!(ApiServer::from_url("   ").is_none());
        assert!(ApiServer::from_url("not-a-url").is_none());
        assert!(ApiServer::from_url("https://").is_none());
    }

    #[test]
    fn an_ipv6_server_url_keeps_its_host_and_port() {
        let api = ApiServer::from_url("http://[fd00::1]:8080").unwrap();
        assert_eq!(api.host, "fd00::1");
        assert_eq!(api.port, 8080);
        // The transport layer reports an IPv6 host unbracketed, which is the
        // form `ApiServer::from_url` stores.
        let dns = resolver(&[]);
        assert!(!destination_is_blocked("fd00::1", 8080, Some(&api), &dns));
    }

    #[test]
    fn the_committed_ranges_parse_and_cover_both_families() {
        let ranges = ranges();
        assert!(!ranges.v4.private.is_empty());
        assert!(!ranges.v6.private.is_empty());
        assert!(!ranges.v4.not_global.is_empty(), "100.64.0.0/10 is missing");
    }

    #[test]
    fn a_private_range_exception_is_honoured() {
        // 192.0.0.0/24 is private, but two addresses in it are carved out.
        assert!(is_internal(ip("192.0.0.1")));
        assert!(!is_internal(ip("192.0.0.9")));
        assert!(!is_internal(ip("192.0.0.10")));
    }
}
