#!/usr/bin/env python3
"""Export the Python MCP server's contracts for the Rust port.

The Python code stays the single source of truth for two things the Rust server
must reproduce byte for byte:

- `DocumentSource` values, used to validate the `source_types` filter.
- The JSON Schema FastMCP derives for each tool's arguments, which MCP clients
  read to decide how to call it.

`backend/tests/unit/lumen/mcp_server/test_exported_contracts.py` fails if the
committed files drift from the Python definitions.

Run from the repo root:

    uv run python backend/native/lumen-mcp-server/scripts/export_contracts.py
"""

from __future__ import annotations

import asyncio
import json
import os
import sys
from pathlib import Path

# Same idiom as backend/scripts/*.py: put `backend/` on the path.
_CRATE_DIR = Path(os.path.abspath(__file__)).parent.parent
sys.path.append(str(_CRATE_DIR.parent.parent))

DATA_DIR = _CRATE_DIR / "data"
DOCUMENT_SOURCES_PATH = DATA_DIR / "document_sources.json"
TOOL_SCHEMAS_PATH = DATA_DIR / "tool_schemas.json"


def document_source_values() -> list[str]:
    """Every DocumentSource value, sorted so the file has a stable diff."""
    from lumen.configs.constants import DocumentSource

    return sorted(member.value for member in DocumentSource)


async def tool_schemas() -> dict[str, dict]:
    """Each tool's argument schema, keyed by tool name."""
    # Importing the API module registers the tools through their decorators.
    from lumen.mcp_server.api import mcp_server

    tools = await mcp_server._list_tools()
    return {tool.name: tool.parameters for tool in sorted(tools, key=lambda t: t.name)}


def write(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


async def main() -> int:
    sources = document_source_values()
    write(DOCUMENT_SOURCES_PATH, sources)
    print(f"wrote {len(sources)} sources to {DOCUMENT_SOURCES_PATH}")

    schemas = await tool_schemas()
    write(TOOL_SCHEMAS_PATH, schemas)
    print(f"wrote {len(schemas)} tool schemas to {TOOL_SCHEMAS_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
