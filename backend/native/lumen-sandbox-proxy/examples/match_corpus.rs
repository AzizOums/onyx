//! Runs the Rust matcher over a corpus of requests and prints its verdicts.
//!
//! Driven by `scripts/matcher_parity_check.py`, which builds the corpus, runs
//! the same cases through the Python matcher, and compares. This binary exists
//! only so the two implementations can be pointed at identical inputs.
//!
//! Reads a JSON corpus on stdin, writes a JSON array of verdicts on stdout:
//!
//! ```text
//! {"cases": [{"app_type": "GITHUB", "app_id": 1, "method": "GET",
//!             "path": "/user", "body": null, "stored": {},
//!             "is_available": true}]}
//! ```

use std::collections::HashMap;
use std::io::Read;

use lumen_sandbox_proxy::catalog::{app_for_type, Scope};
use lumen_sandbox_proxy::matching::{
    apply_credential_gate, recognize_actions, EndpointPolicy, GatedAppKind, GatedTarget,
    ProxiedRequest,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    app_type: String,
    app_id: i32,
    method: String,
    path: String,
    /// The raw request body as a UTF-8 string, or null.
    body: Option<String>,
    /// The raw request body as bytes, for the cases that are not UTF-8.
    /// Takes precedence over `body` when present.
    #[serde(default)]
    body_bytes: Option<Vec<u8>>,
    /// The admin's per-action policy overrides.
    #[serde(default)]
    stored: HashMap<String, EndpointPolicy>,
    is_available: bool,
}

fn main() {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .expect("a corpus on stdin");
    let corpus: Corpus = serde_json::from_str(&input).expect("a valid corpus");

    let verdicts: Vec<serde_json::Value> = corpus
        .cases
        .iter()
        .map(|case| {
            let app = app_for_type(&case.app_type)
                .unwrap_or_else(|| panic!("unknown app_type {}", case.app_type));
            // The corpus is built on a self-hosted deployment, so the Python
            // side offers the full catalog and this side must too.
            let catalog = app.endpoint_catalog(Scope::SelfHosted);
            let target = GatedTarget {
                kind: GatedAppKind::ExternalApp,
                id: case.app_id,
                app_name: app.app_name.clone(),
            };
            // The corpus carries paths as the interception layer reports
            // them, query string and all, so both sides normalise the same way.
            let body = case
                .body_bytes
                .clone()
                .or_else(|| case.body.as_ref().map(|b| b.as_bytes().to_vec()));
            // The corpus carries paths as the interception layer reports them,
            // query string and all, so both sides normalise the same way.
            let request = ProxiedRequest::from_http(&case.method, &case.path, body);

            let matched = recognize_actions(&catalog, &case.stored, &target, &request);
            let gated = apply_credential_gate(&target, &request, matched, case.is_available);
            match gated {
                None => serde_json::Value::Null,
                Some(actions) => serde_json::to_value(&actions).expect("serializable"),
            }
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string(&verdicts).expect("serializable verdicts")
    );
}
