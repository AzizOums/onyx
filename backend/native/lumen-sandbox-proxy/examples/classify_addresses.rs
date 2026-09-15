//! Classifies addresses as internal or not, for the lockdown parity check.
//!
//! Driven by `scripts/lockdown_parity_check.py`, which sends the same addresses
//! through Python's `ipaddress` and compares. Reads one address per line on
//! stdin, writes `1` (internal) or `0` per line on stdout.

use std::io::{BufRead, Write};
use std::net::IpAddr;

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let line = line.expect("readable stdin");
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        let address: IpAddr = text
            .parse()
            .unwrap_or_else(|error| panic!("unparsable address {text:?}: {error}"));
        let internal = lumen_sandbox_proxy::is_internal(address);
        writeln!(out, "{}", u8::from(internal)).expect("writable stdout");
    }
}
