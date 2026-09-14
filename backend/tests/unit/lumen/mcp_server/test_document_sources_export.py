"""The Rust MCP server compiles in a copy of DocumentSource; keep them in sync."""

import json
from pathlib import Path

from lumen.configs.constants import DocumentSource

_EXPORT_PATH = (
    Path(__file__).resolve().parents[4]
    / "native"
    / "lumen-mcp-server"
    / "data"
    / "document_sources.json"
)
_GENERATOR = "backend/native/lumen-mcp-server/scripts/export_document_sources.py"


def test_exported_document_sources_match_the_enum() -> None:
    assert _EXPORT_PATH.is_file(), f"{_EXPORT_PATH} is missing; run {_GENERATOR}"

    exported = json.loads(_EXPORT_PATH.read_text(encoding="utf-8"))
    expected = sorted(member.value for member in DocumentSource)

    assert exported == expected, (
        "The Rust MCP server's DocumentSource copy is stale. "
        f"Regenerate it with: uv run python {_GENERATOR}"
    )
