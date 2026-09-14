"""Resource registrations for the Lumen MCP server."""

# Import resource modules so decorators execute when the package loads.
from lumen.mcp_server.resources import (
    agents,  # noqa: F401
    document_sets,  # noqa: F401
    indexed_sources,  # noqa: F401
)
