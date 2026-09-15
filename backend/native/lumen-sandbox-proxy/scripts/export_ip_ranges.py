#!/usr/bin/env python3
"""Export the address ranges Python calls not-globally-reachable.

The proxy's egress lockdown denies any destination that is, or resolves to, an
address `ipaddress.ip_address(...).is_global` reports as False. That verdict
comes from tables inside the standard library, which track the IANA
special-purpose registries and change between Python releases.

The Rust lockdown has to deny exactly the same set. Hardcoding the tables would
let a Python upgrade widen one side and not the other — a new internal range the
Rust proxy would relay a sandbox to. So they are exported here and
`backend/tests/unit/lumen/sandbox_proxy/test_exported_ip_ranges.py` fails when
the two drift.

Run from the repo root:

    uv run python backend/native/lumen-sandbox-proxy/scripts/export_ip_ranges.py
"""

from __future__ import annotations

import ipaddress
import json
import os
import sys
from pathlib import Path
from typing import Any

_CRATE_DIR = Path(os.path.abspath(__file__)).parent.parent
RANGES_PATH = _CRATE_DIR / "data" / "ip_special_ranges.json"


def build_ranges() -> dict[str, Any]:
    v4 = ipaddress.IPv4Address._constants  # noqa: SLF001
    v6 = ipaddress.IPv6Address._constants  # noqa: SLF001
    return {
        # Carried so a mismatch names the Python that produced the file.
        "python_version": "%d.%d" % sys.version_info[:2],
        "v4": {
            "private": [str(net) for net in v4._private_networks],  # noqa: SLF001
            "private_exceptions": [
                str(net)
                for net in v4._private_networks_exceptions  # noqa: SLF001
            ],
            # Neither private nor global: `is_global` excludes it explicitly.
            "not_global": [str(v4._public_network)],  # noqa: SLF001
        },
        "v6": {
            "private": [str(net) for net in v6._private_networks],  # noqa: SLF001
            "private_exceptions": [
                str(net)
                for net in v6._private_networks_exceptions  # noqa: SLF001
            ],
            "not_global": [],
        },
    }


def main() -> int:
    # An explicit path lets the drift test run this exact command into a temp
    # file rather than re-importing the builder.
    destination = Path(sys.argv[1]) if len(sys.argv) > 1 else RANGES_PATH
    ranges = build_ranges()
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(ranges, indent=2) + "\n", encoding="utf-8")
    counts = {family: len(ranges[family]["private"]) for family in ("v4", "v6")}
    print(f"wrote {destination} (python {ranges['python_version']}, {counts})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
