#!/usr/bin/env python3
"""Check that the Rust CA bootstrap interoperates with the Python one.

A deployment already running the Python proxy has a CA on disk, and every
sandbox trust store already contains it. The Rust proxy must load that exact CA;
regenerating would orphan every sandbox at once.

This writes a CA with the Python code, hands it to the Rust tests, then does the
reverse: loads a Rust-written CA with the Python code and compares the two
certificates field by field.

Run from the repo root:

    uv run python backend/native/lumen-sandbox-proxy/scripts/ca_interop_check.py
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

# Same idiom as backend/scripts/*.py: put `backend/` on the path.
_CRATE_DIR = Path(os.path.abspath(__file__)).parent.parent
_NATIVE_DIR = _CRATE_DIR.parent
sys.path.append(str(_NATIVE_DIR.parent))

from cryptography import x509  # noqa: E402
from cryptography.x509.oid import ExtensionOID  # noqa: E402

from lumen.sandbox_proxy.ca import CABootstrap  # noqa: E402
from lumen.sandbox_proxy.ca_docker import FileCAStore  # noqa: E402

# Smaller than the 4096 the proxy uses in production. The key size is not what
# this checks, and 4096 makes the run take minutes.
KEY_SIZE_BITS = 2048


def describe(cert_pem: bytes) -> dict[str, object]:
    """The fields both implementations must agree on."""
    cert = x509.load_pem_x509_certificate(cert_pem)

    basic = cert.extensions.get_extension_for_oid(ExtensionOID.BASIC_CONSTRAINTS)
    usage = cert.extensions.get_extension_for_oid(ExtensionOID.KEY_USAGE)

    # The Python bootstrap adds this one explicitly, so it is part of the shape
    # a loaded CA is expected to have.
    try:
        cert.extensions.get_extension_for_oid(ExtensionOID.SUBJECT_KEY_IDENTIFIER)
        has_ski = True
    except x509.ExtensionNotFound:
        has_ski = False

    return {
        "subject": cert.subject.rfc4514_string(),
        "issuer": cert.issuer.rfc4514_string(),
        "self_signed": cert.subject == cert.issuer,
        "basic_constraints_critical": basic.critical,
        "ca": basic.value.ca,
        "path_length": basic.value.path_length,
        "key_usage_critical": usage.critical,
        "key_cert_sign": usage.value.key_cert_sign,
        "crl_sign": usage.value.crl_sign,
        "digital_signature": usage.value.digital_signature,
        "key_encipherment": usage.value.key_encipherment,
        "subject_key_identifier": has_ski,
    }


def python_ca(directory: Path) -> bytes:
    """Write a CA the way the running proxy does, and return its certificate."""
    store = FileCAStore(root=directory)
    bootstrap = CABootstrap(
        store=store,
        pem_path=directory / "mitmproxy-ca.pem",
        key_size_bits=KEY_SIZE_BITS,
    )
    return bootstrap.ensure_ca().cert_pem


def rust_ca(directory: Path) -> bytes:
    """Ask the Rust bootstrap for a CA in `directory`, and return its certificate."""
    binary = _NATIVE_DIR / "target" / "debug" / "examples" / "bootstrap_ca"
    if not binary.is_file():
        raise SystemExit(
            f"missing {binary}; build it with:\n"
            f"  cargo build --manifest-path {_NATIVE_DIR}/Cargo.toml "
            "-p lumen-sandbox-proxy --example bootstrap_ca"
        )
    subprocess.run(  # noqa: S603
        [str(binary), str(directory), str(KEY_SIZE_BITS)],
        check=True,
        capture_output=True,
        text=True,
    )
    return (directory / "ca.crt").read_bytes()


def run_rust_tests(fixture: Path) -> bool:
    env = dict(os.environ, LUMEN_CA_INTEROP_DIR=str(fixture))
    result = subprocess.run(  # noqa: S603
        [
            "cargo",
            "test",
            "--manifest-path",
            str(_NATIVE_DIR / "Cargo.toml"),
            "-p",
            "lumen-sandbox-proxy",
            "--test",
            "ca_interop",
        ],
        env=env,
        capture_output=True,
        text=True,
    )
    print(result.stdout[-2000:] if result.returncode else "rust: ok")
    if result.returncode:
        print(result.stderr[-2000:])
    return result.returncode == 0


def main() -> int:
    failures: list[str] = []

    with tempfile.TemporaryDirectory() as raw:
        from_python = Path(raw) / "from-python"
        from_rust = Path(raw) / "from-rust"
        from_python.mkdir()
        from_rust.mkdir()

        print("generating a CA with the Python proxy code...")
        python_cert = python_ca(from_python)

        print("loading it from Rust...")
        if not run_rust_tests(from_python):
            failures.append("Rust could not load the Python CA")

        print("generating a CA with the Rust bootstrap...")
        rust_cert = rust_ca(from_rust)

        print("loading it from Python...")
        try:
            x509.load_pem_x509_certificate(rust_cert)
        except Exception as error:  # noqa: BLE001
            failures.append(f"Python could not parse the Rust CA: {error}")

        python_fields = describe(python_cert)
        rust_fields = describe(rust_cert)

        print()
        print(f"{'field':30} {'python':<24} rust")
        for field in python_fields:
            same = python_fields[field] == rust_fields[field]
            mark = " " if same else "  <-- differs"
            print(
                f"{field:30} {str(python_fields[field]):<24} {rust_fields[field]}{mark}"
            )
            if not same:
                failures.append(
                    f"{field}: {python_fields[field]!r} vs {rust_fields[field]!r}"
                )

    print()
    if failures:
        print(f"divergences: {len(failures)}")
        for failure in failures:
            print(f"  - {failure}")
        return 1
    print("the two CA implementations agree on every checked field")
    return 0


if __name__ == "__main__":
    sys.exit(main())
