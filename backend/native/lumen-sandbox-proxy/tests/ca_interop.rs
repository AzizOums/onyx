//! Interoperability with the Python CA bootstrap.
//!
//! A deployment that already runs the Python proxy has a CA on disk and every
//! sandbox trust store already contains it. The Rust proxy has to load that
//! exact CA — regenerating would orphan every sandbox — so this reads a CA the
//! Python code wrote and checks it is accepted and materialized unchanged.
//!
//! Driven by `scripts/ca_interop_check.py`, which writes the fixture and sets
//! `LUMEN_CA_INTEROP_DIR`. Without that variable the tests report a skip.

use std::path::PathBuf;

use lumen_sandbox_proxy::ca::{validate_certificate, CaStore};
use lumen_sandbox_proxy::{CaBootstrap, FileCaStore};

fn fixture_dir() -> Option<PathBuf> {
    std::env::var("LUMEN_CA_INTEROP_DIR")
        .ok()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[test]
fn a_ca_written_by_the_python_proxy_loads_and_validates() {
    let Some(directory) = fixture_dir() else {
        eprintln!("skipped: set LUMEN_CA_INTEROP_DIR (see scripts/ca_interop_check.py)");
        return;
    };

    let store = FileCaStore::new(&directory);
    let (cert_pem, key_pem) = store
        .load()
        .expect("the Python-written store reads cleanly")
        .expect("the fixture contains a CA");

    validate_certificate(&cert_pem).expect("the Python CA is accepted");

    // The structure the proxy depends on: a constrained CA that can sign
    // certificates, which is what makes TLS interception work at all.
    let (_, pem) = x509_parser::pem::parse_x509_pem(&cert_pem).expect("valid PEM");
    let certificate = pem.parse_x509().expect("valid X.509");

    let basic = certificate
        .basic_constraints()
        .expect("basic constraints parse")
        .expect("basic constraints present");
    assert!(basic.value.ca);
    assert_eq!(basic.value.path_len_constraint, Some(0));

    let usage = certificate
        .key_usage()
        .expect("key usage parses")
        .expect("key usage present");
    assert!(usage.value.key_cert_sign(), "the CA must be able to sign");

    assert!(key_pem.starts_with(b"-----BEGIN PRIVATE KEY-----"));
}

#[test]
fn the_existing_ca_is_reused_rather_than_replaced() {
    let Some(directory) = fixture_dir() else {
        eprintln!("skipped: set LUMEN_CA_INTEROP_DIR (see scripts/ca_interop_check.py)");
        return;
    };

    let store = FileCaStore::new(&directory);
    let (before_cert, before_key) = store.load().unwrap().expect("a CA is present");

    let output = tempfile::tempdir().expect("a temp dir");
    let materialized = CaBootstrap::new(
        FileCaStore::new(&directory),
        output.path().join("mitmproxy-ca.pem"),
    )
    .ensure_ca()
    .expect("the bootstrap succeeds against an existing CA");

    // This is the property that protects every sandbox already trusting it.
    assert_eq!(
        materialized.cert_pem, before_cert,
        "the bootstrap replaced a CA that sandboxes already trust"
    );
    assert_eq!(materialized.key_pem, before_key);

    let (after_cert, after_key) = store.load().unwrap().expect("a CA is still present");
    assert_eq!(after_cert, before_cert, "the store was rewritten");
    assert_eq!(after_key, before_key);

    // And the single PEM the proxy reads is key-then-certificate.
    let written = std::fs::read(&materialized.pem_path).expect("the PEM exists");
    let mut expected = before_key.clone();
    expected.push(b'\n');
    expected.extend_from_slice(&before_cert);
    assert_eq!(written, expected);
}
