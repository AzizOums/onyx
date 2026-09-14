#!/usr/bin/env python3
"""Export DocumentSource values for the Rust MCP server.

The Python enum stays the single source of truth. This writes the values the
Rust crate compiles in, and
`backend/tests/unit/lumen/mcp_server/test_document_sources_export.py` fails if
the two drift.

Run from the repo root:

    uv run python backend/native/lumen-mcp-server/scripts/export_document_sources.py
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

# Same idiom as backend/scripts/*.py: put `backend/` on the path.
_CRATE_DIR = Path(os.path.abspath(__file__)).parent.parent
sys.path.append(str(_CRATE_DIR.parent.parent))

from lumen.configs.constants import DocumentSource  # noqa: E402

OUTPUT_PATH = _CRATE_DIR / "data" / "document_sources.json"


def document_source_values() -> list[str]:
    """Every DocumentSource value, sorted so the file has a stable diff."""
    return sorted(member.value for member in DocumentSource)


def render() -> str:
    return json.dumps(document_source_values(), indent=2) + "\n"


def main() -> int:
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_text(render(), encoding="utf-8")
    print(f"wrote {len(document_source_values())} sources to {OUTPUT_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
