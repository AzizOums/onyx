# Native code

The repository's Rust backend code, as a cargo workspace.

| Crate | What it is | Status |
| --- | --- | --- |
| `lumen_text` | A Python extension module replacing the HTML-to-text and text-cleanup hot path | Optional; off unless `LUMEN_NATIVE_TEXT=true` |
| `lumen-mcp-server` | The MCP server, a standalone binary replacing `backend/lumen/mcp_server/` | Built and tested; the chart still runs the Python server by default |

Both are additive: with neither built, the Python code runs unchanged.

```bash
cargo build --release --manifest-path backend/native/Cargo.toml
cargo test --manifest-path backend/native/Cargo.toml
```

`cargo test` needs `--no-default-features` for `lumen_text`, so that its test
binary can link against libpython.

## Language policy

Rust is the target language for backend code. The web frontend stays on
Next.js and React.

| Area | Language | Note |
| --- | --- | --- |
| CPU-bound work inside the Python process (parsing, chunking, text cleanup) | Rust | Links into CPython through PyO3. No network hop, no new service, no serialization cost. |
| Standalone backend services (proxies, gateways, HTTP services) | Rust | Predictable latency and memory, no GIL. |
| Web frontend | TypeScript | Next.js and React. Not a migration target. |
| Python still to migrate | Python | The FastAPI app, the Celery workers, and the connectors. Moving each one is its own project. |

`model_server` is the one backend component that stays on Python: it runs torch
and the HuggingFace stack, which have no Rust equivalent.

Rust is already in the tree (`desktop/src-tauri`), so the native code adds no
new toolchain.

### Order of migration

The MCP server moved first because it reads no database. Everything after it is
gated on `backend/lumen/db`.

1. `lumen_text` — done, opt-in
2. `lumen-mcp-server` — done, opt-in (`lumen-mcp-server/README.md`)
3. the database layer — the real prerequisite
4. `sandbox_proxy` — its pure layers are done, the rest is blocked, see below
5. `api_server` and the Celery workers — not startable before step 3

### What of `sandbox_proxy` is ported

Three pieces, each one that needs no database, no network and no TLS engine:

| Piece | What it decides |
| --- | --- |
| `ca`, `ca_file` | The authority the proxy signs intercepted connections with |
| `matching` | What a request is allowed to do, against the action catalog |
| `lockdown` | Where a request may go, by address rather than by hostname |

The two decision layers are the security boundary, so neither is trusted on a
reading of the Python. Each is checked against it over a generated corpus, and
each check is itself checked by injecting a bug and watching it fail. See
`../VERIFICATION.md`.

The catalog and the address tables are **generated from Python**, not
re-declared: the providers and `ipaddress` stay the single source of truth, and
a drift test fails when a copy falls behind.

```bash
uv run python backend/native/lumen-sandbox-proxy/scripts/export_catalog.py
uv run python backend/native/lumen-sandbox-proxy/scripts/export_ip_ranges.py
```

### Why the rest of `sandbox_proxy` is blocked

The transport and the approval pipeline are another matter. The proxy reads as
a standalone binary of 4,600 lines; its dependency closure is not.

| What it needs | Size |
| --- | --- |
| `lumen/sandbox_proxy` itself | 4,600 lines |
| `lumen/external_apps` — action matching, credential injection, OAuth refresh | 3,739 lines |
| A slice of `lumen/db`, including **writes**: notifications and action approvals | — |
| Multi-tenant Postgres: per-tenant schema translation plus shard routing | — |
| The credential encryption used for `Sandbox.encrypted_pat` | — |
| A MITM TLS engine, today `mitmproxy` | — |

So a straight port of the rest means the database layer first, which is step 3.
What is ported above is the part with no such dependency.

There is a second path that skips it: have the proxy ask the API server for what
it needs, exactly as the MCP server does. Resolving a sandbox by IP, evaluating
the gate for one request, resolving injection headers and recording an approval
are four or five endpoints. The proxy would then hold no database credentials
and no decryption key — which is worth something on its own for a component that
terminates sandbox TLS.

That path changes the architecture, so it is a decision to take deliberately
rather than a detail of the port.

### The size of step 3

| | |
| --- | --- |
| `lumen/db` | 103 files, 43,438 lines |
| `models.py` | 7,461 lines, 163 SQLAlchemy models |
| Alembic revisions | 447 |
| Engine | per-tenant schema translation and shard routing |

The risk here is not the line count. It is that a Rust schema and the Alembic
history are two definitions of the same tables, and two definitions drift. The
`lumen_text` and MCP ports both handle this by generating the Rust side from the
Python definition and failing a test when the copy goes stale; whatever is built
here needs an equivalent, or the migration trades a slow backend for silent data
corruption.


## lumen_text

Replaces the HTML-to-text and text-cleanup primitives used by every connector
and by the indexing pipeline.

| Python function | Module |
| --- | --- |
| `parse_html_page_basic` | `lumen/file_processing/html_utils.py` |
| `web_html_cleanup` | `lumen/file_processing/html_utils.py` |
| `clean_text` | `lumen/utils/text_processing.py` |
| `shared_precompare_cleanup` | `lumen/utils/text_processing.py` |

### Build

```bash
backend/native/build.sh          # needs a Rust toolchain: https://rustup.rs
export LUMEN_NATIVE_TEXT=true    # opt in
```

The build produces an abi3 module that works on CPython 3.11 and later. Without
`LUMEN_NATIVE_TEXT=true` the module is never imported.

### Tests

```bash
cd backend/native/lumen_text && cargo test --no-default-features
uv run pytest backend/tests/unit/lumen/file_processing/test_native_text_parity.py
```

The Rust tests need `--no-default-features` so the test binary can link against
libpython. The pytest file runs each input through both paths and asserts the
results are identical; it skips when the module is not built.

### Parity

The two paths agree byte-for-byte on well-formed HTML. Measured on this
checkout:

- The fixed-case pytest suite: 164 checks, 0 divergences.
- 3 000 generated well-formed documents: 0 divergences.

They disagree on malformed markup, because lxml (libxml2) and html5ever apply
different recovery rules:

- `<tr>`, `<td>`, `<th>` outside a `<table>`
- block content or another heading inside `<h1>`-`<h6>`
- tags inside `<script>`

On a generator that produces such markup deliberately, 27% of documents differ.
Real pages are mostly well-formed, but crawled HTML is not guaranteed to be,
which is why the flag is off by default. Validate against your own crawl before
turning it on.

To reproduce these numbers, or to check the native path against your own crawl
corpus before turning it on, see `parity/README.md`.

Three lxml behaviours the Rust code reproduces on purpose, because the Python
output depends on them:

1. A text node made only of ASCII whitespace collapses to one character: `"\n"`
   when it holds a line break, `" "` otherwise. Not applied under `<pre>` or
   `<textarea>`.
2. Text inside `<script>`, `<style>`, `<template>`, `<rt>` and `<rp>` reports as
   empty through BeautifulSoup, so it never reaches the output.
3. `\r` normalizes to `\n` before rule 1 applies.

### Measured speedup

`parse_html_page_basic` and `web_html_cleanup`, against bs4 + lxml:

| Input | parse_html_page_basic | web_html_cleanup |
| --- | --- | --- |
| 1 KB page | 33x | 52x |
| 10 KB page | 37x | 50x |
| 58 KB page | 37x | 54x |

`clean_text` is 23x faster and `shared_precompare_cleanup` 12x on a 37 KB input.

Reproduce with `parity/bench.py`.

Both HTML functions release the GIL while they work, so threaded Celery workers
gain concurrency as well as speed.
