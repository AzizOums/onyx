#!/usr/bin/env python3
"""Check that the Rust lockdown calls the same addresses internal as Python.

`destination_is_blocked` denies anything `ipaddress.ip_address(...).is_global`
reports as False. That verdict is the proxy's only defence against a sandbox
relaying to a database, a cache, or the cloud metadata endpoint, so the Rust
port has to agree on every address — not on a sample of the ones somebody
thought of.

This drives both implementations over the boundaries of every special range
(the address below, at, inside, at the end of, and above each block) and a large
random sample, and reports every address they disagree on.

Run from the repo root:

    cargo build --manifest-path backend/native/Cargo.toml \
        -p lumen-sandbox-proxy --example classify_addresses
    uv run python backend/native/lumen-sandbox-proxy/scripts/lockdown_parity_check.py
"""

from __future__ import annotations

import ipaddress
import json
import os
import random
import subprocess
import sys
from pathlib import Path

_CRATE_DIR = Path(os.path.abspath(__file__)).parent.parent
_NATIVE_DIR = _CRATE_DIR.parent
RANGES_PATH = _CRATE_DIR / "data" / "ip_special_ranges.json"

RANDOM_V4 = 200_000
RANDOM_V6 = 200_000
SEED = 20240915


def boundary_addresses() -> list[str]:
    """Around every block edge, where an off-by-one mask lives."""
    ranges = json.loads(RANGES_PATH.read_text(encoding="utf-8"))
    addresses: list[str] = []
    for family in ("v4", "v6"):
        blocks = (
            ranges[family]["private"]
            + ranges[family]["private_exceptions"]
            + ranges[family]["not_global"]
        )
        for block in blocks:
            network = ipaddress.ip_network(block)
            first = int(network.network_address)
            last = int(network.broadcast_address)
            limit = (1 << (32 if family == "v4" else 128)) - 1
            candidates = {
                first - 1,
                first,
                first + 1,
                (first + last) // 2,
                last - 1,
                last,
                last + 1,
            }
            addresses.extend(
                str(ipaddress.ip_address(value))
                for value in candidates
                if 0 <= value <= limit
            )
    return addresses


def random_addresses() -> list[str]:
    rng = random.Random(SEED)
    addresses = [
        str(ipaddress.IPv4Address(rng.getrandbits(32))) for _ in range(RANDOM_V4)
    ]
    for _ in range(RANDOM_V6):
        # Half uniform across the space, half concentrated in the low bits where
        # the special ranges and the IPv4-mapped block live.
        if rng.random() < 0.5:
            value = rng.getrandbits(128)
        else:
            value = rng.getrandbits(48)
        addresses.append(str(ipaddress.IPv6Address(value)))
    return addresses


def named_addresses() -> list[str]:
    """The ones a reader will look for by name."""
    return [
        "8.8.8.8",
        "1.1.1.1",
        "10.0.0.1",
        "127.0.0.1",
        "169.254.169.254",  # cloud metadata
        "100.64.0.1",  # carrier-grade NAT
        "192.0.0.9",  # a carve-out inside a private block
        "255.255.255.255",
        "::1",
        "::",
        "fc00::1",
        "fe80::1",
        "2606:4700::1111",
        "::ffff:10.0.0.1",  # IPv4-mapped internal
        "::ffff:8.8.8.8",  # IPv4-mapped public
        "::ffff:169.254.169.254",
        "64:ff9b:1::1",
        "2001:db8::1",
    ]


def rust_verdicts(addresses: list[str]) -> list[bool]:
    binary = _NATIVE_DIR / "target" / "debug" / "examples" / "classify_addresses"
    if not binary.is_file():
        raise SystemExit(
            f"missing {binary}; build it with:\n"
            f"  cargo build --manifest-path {_NATIVE_DIR}/Cargo.toml "
            "-p lumen-sandbox-proxy --example classify_addresses"
        )
    result = subprocess.run(  # noqa: S603
        [str(binary)],
        input="\n".join(addresses),
        capture_output=True,
        text=True,
        check=True,
    )
    return [line == "1" for line in result.stdout.split()]


def main() -> int:
    addresses = named_addresses() + boundary_addresses() + random_addresses()
    print(f"classifying {len(addresses)} addresses")

    expected = [not ipaddress.ip_address(a).is_global for a in addresses]
    actual = rust_verdicts(addresses)

    if len(expected) != len(actual):
        print(f"the Rust runner returned {len(actual)} verdicts for {len(expected)}")
        return 1

    divergences = [
        (address, want, got)
        for address, want, got in zip(addresses, expected, actual, strict=True)
        if want != got
    ]
    internal = sum(expected)
    print(f"internal by Python: {internal}; public: {len(addresses) - internal}")

    if divergences:
        print(f"\ndivergences: {len(divergences)} (showing up to 20)")
        for address, want, got in divergences[:20]:
            print(f"  - {address}: python internal={want}, rust internal={got}")
        return 1
    print("the two implementations agree on every address")
    return 0


if __name__ == "__main__":
    sys.exit(main())
