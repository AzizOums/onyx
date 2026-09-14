#!/usr/bin/env python3
"""Compare the Rust MCP server against the Python one, response by response.

Both servers are pointed at the same mock Lumen API and asked the same MCP
requests; every answer is compared. This is the evidence for the cutover.

Run from the repo root:

    cargo build --release --manifest-path backend/native/Cargo.toml
    uv run python backend/native/lumen-mcp-server/scripts/parity_check.py

Exit status is non-zero when anything differs.
"""

from __future__ import annotations

import asyncio
import difflib
import json
import os
import socket
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

_CRATE_DIR = Path(os.path.abspath(__file__)).parent.parent
_BACKEND_DIR = _CRATE_DIR.parent.parent
_REPO_ROOT = _BACKEND_DIR.parent
sys.path.append(str(_BACKEND_DIR))

RUST_BINARY = (
    _REPO_ROOT / "backend" / "native" / "target" / "release" / "lumen-mcp-server"
)

# What the mock API answers for each endpoint the MCP server calls.
FIXTURES: dict[str, tuple[int, Any]] = {
    "/me": (200, {"id": "user-1", "email": "user@example.com"}),
    "/manage/indexed-sources": (200, {"sources": ["jira", "github", "google_drive"]}),
    "/manage/document-set": (
        200,
        [
            {"id": 2, "name": "Zeta", "description": "z", "cc_pair_summaries": []},
            {"id": 1, "name": "Alpha", "description": None},
        ],
    ),
    "/persona": (
        200,
        [
            {"id": 9, "name": "Support", "description": "help", "tools": []},
            {"id": 3, "name": "Analytics", "description": None},
        ],
    ),
    "/search": (
        200,
        {
            "results": [
                {
                    "citation_id": 1,
                    "title": "Ticket",
                    "content": "body text",
                    "link": "https://jira/PROJ-1",
                    "source_type": "jira",
                    "updated_at": "2025-01-01T00:00:00Z",
                }
            ]
        },
    ),
    "/web-search/search-lite": (
        200,
        {
            "results": [
                {
                    "document_citation_number": 1,
                    "unique_identifier_to_strip_away": None,
                    "type": "web_search",
                    "url": "https://example.com",
                    "title": "Example",
                    "snippet": "snip",
                    "provider_internal_score": 0.9,
                }
            ],
            "provider_type": "exa",
        },
    ),
    "/web-search/open-urls": (
        200,
        {
            "results": [
                {
                    "document_citation_number": 1,
                    "type": "open_url",
                    "content": "page body",
                    "fetched_with": "playwright",
                }
            ],
            "provider_type": None,
        },
    ),
}

# Per-case overrides, applied on top of FIXTURES.
CASES: list[dict[str, Any]] = [
    {
        "name": "tools/list",
        "kind": "list_tools",
    },
    {
        "name": "resources/list",
        "kind": "list_resources",
    },
    {
        "name": "resource indexed_sources",
        "kind": "read_resource",
        "uri": "resource://indexed_sources",
    },
    {
        "name": "resource document_sets",
        "kind": "read_resource",
        "uri": "resource://document_sets",
    },
    {
        "name": "resource agents",
        "kind": "read_resource",
        "uri": "resource://agents",
    },
    {
        "name": "search: plain query",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "status of PROJ-1"},
    },
    {
        "name": "search: filters and cutoff",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {
            "query": "status",
            "source_types": ["JIRA", "not_a_source", "github"],
            "time_cutoff": "2025-11-24T00:00:00Z",
            "skip_query_expansion": True,
        },
    },
    {
        "name": "search: naive cutoff",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "q", "time_cutoff": "2025-11-24T12:34:56.789"},
    },
    {
        "name": "search: unparseable cutoff",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "q", "time_cutoff": "tomorrow"},
    },
    {
        "name": "search: document sets",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "q", "document_set_names": ["Alpha"]},
    },
    {
        "name": "search: agent by exact name",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "q", "agent": "Support"},
    },
    {
        "name": "search: agent case-folded",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "q", "agent": "  support "},
    },
    {
        "name": "search: unknown agent",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "q", "agent": "Nope"},
    },
    {
        "name": "search: agent and document sets together",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "q", "agent": "Support", "document_set_names": ["Alpha"]},
    },
    {
        "name": "search: no indexed sources",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "q"},
        "overrides": {"/manage/indexed-sources": (200, {"sources": []})},
    },
    {
        "name": "search: empty lists",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "q", "source_types": [], "document_set_names": []},
    },
    {
        "name": "search: upstream error",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "q"},
        "overrides": {
            "/search": (400, {"error_code": "BAD_REQUEST", "detail": "query too long"})
        },
    },
    {
        "name": "search: non-json upstream error",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "q"},
        "overrides": {"/search": (502, "<html>nginx</html>")},
    },
    {
        "name": "search: agent lookup fails",
        "kind": "call_tool",
        "tool": "search_indexed_documents",
        "args": {"query": "q", "agent": "Support"},
        "overrides": {"/persona": (403, {"error_code": "FORBIDDEN", "detail": "nope"})},
        "accepted_divergence": (
            "Python interpolates the raw httpx error, which puts the internal API "
            "URL into a message the MCP client shows its user. Rust reports the "
            "API's own detail instead. Deliberate: it drops nothing the caller "
            "needs and stops leaking an internal address."
        ),
    },
    {
        "name": "search_web: default limit",
        "kind": "call_tool",
        "tool": "search_web",
        "args": {"query": "rust"},
    },
    {
        "name": "search_web: explicit limit",
        "kind": "call_tool",
        "tool": "search_web",
        "args": {"query": "rust", "limit": 3},
    },
    {
        "name": "search_web: upstream error",
        "kind": "call_tool",
        "tool": "search_web",
        "args": {"query": "rust"},
        "overrides": {
            "/web-search/search-lite": (
                429,
                {"error_code": "RATE_LIMIT", "detail": "slow down"},
            )
        },
    },
    {
        "name": "open_urls: two urls",
        "kind": "call_tool",
        "tool": "open_urls",
        "args": {"urls": ["https://a", "https://b"]},
    },
    {
        "name": "open_urls: upstream error",
        "kind": "call_tool",
        "tool": "open_urls",
        "args": {"urls": ["https://a"]},
        "overrides": {
            "/web-search/open-urls": (400, {"error_code": "BAD", "detail": "bad url"})
        },
    },
]


class MockApi:
    """Serves FIXTURES, with per-case overrides swapped in between calls."""

    def __init__(self) -> None:
        self.overrides: dict[str, tuple[int, Any]] = {}
        self.requests: list[dict[str, Any]] = []
        self._server: ThreadingHTTPServer | None = None
        self.port = 0

    def _reply_for(self, path: str) -> tuple[int, Any]:
        if path in self.overrides:
            return self.overrides[path]
        return FIXTURES.get(path, (404, {"detail": "no stub"}))

    def start(self) -> None:
        mock = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args: Any) -> None:  # silence the default logging
                pass

            def _respond(self, body: bytes | None = None) -> None:
                status, payload = mock._reply_for(self.path)
                mock.requests.append(
                    {
                        "path": self.path,
                        "authorization": self.headers.get("Authorization"),
                        "body": json.loads(body) if body else None,
                    }
                )
                raw = (
                    payload.encode()
                    if isinstance(payload, str)
                    else json.dumps(payload).encode()
                )
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)

            def do_GET(self) -> None:  # noqa: N802
                self._respond()

            def do_POST(self) -> None:  # noqa: N802
                length = int(self.headers.get("Content-Length") or 0)
                self._respond(self.rfile.read(length) if length else None)

        self._server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.port = self._server.server_address[1]
        threading.Thread(target=self._server.serve_forever, daemon=True).start()

    def stop(self) -> None:
        if self._server is not None:
            self._server.shutdown()

    def take_requests(self) -> list[dict[str, Any]]:
        requests, self.requests = self.requests, []
        return requests


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def wait_for_health(port: int, timeout: float = 30.0) -> None:
    import httpx

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            response = httpx.get(f"http://127.0.0.1:{port}/health", timeout=1.0)
            if response.status_code == 200:
                return
        except Exception:
            pass
        time.sleep(0.2)
    raise RuntimeError(f"server on port {port} never became healthy")


def server_env(api_port: int, mcp_port: int) -> dict[str, str]:
    env = dict(os.environ)
    env.update(
        {
            "MCP_SERVER_ENABLED": "true",
            "MCP_SERVER_HOST": "127.0.0.1",
            "MCP_SERVER_PORT": str(mcp_port),
            "API_SERVER_URL_OVERRIDE_FOR_HTTP_REQUESTS": f"http://127.0.0.1:{api_port}",
            "API_PREFIX": "",
            "DEV_MODE": "false",
            "LUMEN_MCP_LOG": "warn",
        }
    )
    # The Python entry point is run from backend/, which must be importable.
    env["PYTHONPATH"] = str(_BACKEND_DIR)
    # The Python server imports the whole app; keep its logging quiet.
    env["LOG_LEVEL"] = "error"
    return env


async def drive(url: str, case: dict[str, Any]) -> Any:
    """Run one case against one server and return its normalized answer."""
    from fastmcp import Client

    async with Client(f"{url}/", auth="parity-token") as client:
        kind = case["kind"]
        if kind == "list_tools":
            tools = await client.list_tools()
            return [
                {
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.inputSchema,
                }
                for tool in sorted(tools, key=lambda item: item.name)
            ]
        if kind == "list_resources":
            resources = await client.list_resources()
            return [
                {
                    "uri": str(resource.uri),
                    "name": resource.name,
                    "description": resource.description,
                    "mime_type": resource.mimeType,
                }
                for resource in sorted(resources, key=lambda item: str(item.uri))
            ]
        if kind == "read_resource":
            contents = await client.read_resource(case["uri"])
            return [
                {"mime_type": item.mimeType, "text": getattr(item, "text", None)}
                for item in contents
            ]
        if kind == "call_tool":
            result = await client.call_tool(case["tool"], case["args"])
            return result.structured_content or result.data
        raise ValueError(f"unknown case kind: {kind}")


def normalize_requests(requests: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Only the upstream calls a case triggers matter, in order."""
    return [
        {"path": request["path"], "body": request["body"]}
        for request in requests
        if request["path"] != "/me"
    ]


def report_diff(label: str, python_value: Any, rust_value: Any) -> None:
    """Print a line diff so a long payload stays readable."""
    print("-" * 72)
    print(f"DIFFERENT {label}")
    left = json.dumps(python_value, indent=2, sort_keys=True).splitlines()
    right = json.dumps(rust_value, indent=2, sort_keys=True).splitlines()
    diff = list(difflib.unified_diff(left, right, "python", "rust", lineterm="", n=1))
    for line in diff[:60]:
        print(f"  {line}")
    if len(diff) > 60:
        print(f"  ... {len(diff) - 60} more diff lines")


async def main() -> int:
    if not RUST_BINARY.is_file():
        print(f"missing {RUST_BINARY}; build it with:")
        print("  cargo build --release --manifest-path backend/native/Cargo.toml")
        return 1

    mock = MockApi()
    mock.start()

    rust_port = free_port()
    python_port = free_port()

    rust = subprocess.Popen(  # noqa: S603
        [str(RUST_BINARY)],
        env=server_env(mock.port, rust_port),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    python = subprocess.Popen(  # noqa: S603
        [sys.executable, "lumen/mcp_server_main.py"],
        cwd=str(_BACKEND_DIR),
        env=server_env(mock.port, python_port),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    failures: list[str] = []
    accepted_count = 0
    try:
        wait_for_health(rust_port)
        wait_for_health(python_port)

        rust_url = f"http://127.0.0.1:{rust_port}"
        python_url = f"http://127.0.0.1:{python_port}"

        for case in CASES:
            mock.overrides = case.get("overrides", {})

            mock.take_requests()
            python_answer = await drive(python_url, case)
            python_requests = normalize_requests(mock.take_requests())

            rust_answer = await drive(rust_url, case)
            rust_requests = normalize_requests(mock.take_requests())

            accepted = case.get("accepted_divergence")
            if python_answer != rust_answer:
                if accepted:
                    accepted_count += 1
                    print(f"accepted divergence  {case['name']}")
                    print(f"    {accepted}")
                else:
                    failures.append(case["name"])
                    report_diff(f"ANSWER  {case['name']}", python_answer, rust_answer)
            elif python_requests != rust_requests:
                failures.append(case["name"])
                report_diff(
                    f"UPSTREAM CALLS  {case['name']}", python_requests, rust_requests
                )
            else:
                print(f"ok  {case['name']}")
    finally:
        rust.terminate()
        python.terminate()
        mock.stop()

    print()
    print(
        f"cases: {len(CASES)}   identical: {len(CASES) - len(failures) - accepted_count}"
        f"   accepted divergences: {accepted_count}   unexpected: {len(failures)}"
    )
    for name in failures:
        print(f"  - {name}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
