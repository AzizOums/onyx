# Before this branch goes anywhere near production

What is verified, what is not, and where each unverified thing breaks if it is
wrong. Nothing on this branch is switched on by default — every piece is behind a
flag or a Helm value that still points at Python.

## Status at a glance

| Component | State | Default |
| --- | --- | --- |
| `lumen_text` | Built, tested, parity measured | Off (`LUMEN_NATIVE_TEXT`) |
| `lumen-mcp-server` | Built, tested, parity measured | Off (`mcpServer.runtime: python`) |
| `lumen-db` row structs | Generated, drift test verified | Not wired to anything |
| `lumen-db` connection layer | Tested against a real PostgreSQL 16 | Not wired to anything |
| `sandbox_proxy` CA bootstrap | Ported, interop with Python proven | Not wired to anything |
| `sandbox_proxy` everything else | Not started | — |
| `api_server`, Celery workers | Not started | — |

## Verified, and how

These were run, not assumed.

| Claim | Evidence |
| --- | --- |
| `lumen_text` matches bs4 on well-formed HTML | 167 pytest cases and 3,000 generated documents, 0 divergences |
| The Rust MCP server matches the Python one | 24 cases through the real MCP protocol: 23 identical, 1 accepted divergence, 0 unexpected |
| The generated row structs match the models | Drift injected into the generated file, test observed failing, then restored |
| Tenant scoping isolates schemas | 5 integration tests against a live PostgreSQL 16.13, including cross-tenant reuse |
| A sharded deployment is refused, not misrouted | Unit test asserts `Database::connect` errors |
| The Rust proxy loads a CA the Python proxy wrote | Interop check, 12 certificate fields identical both ways |

## Not verified — and what breaks

### The MCP server image has never been built

No Docker daemon in the environment it was written in. `cargo build --release`
works; the Dockerfile has never run.

**Breaks as:** the image fails to build in CI, or builds and crashes on start.
Loud and immediate, not subtle.

```bash
docker buildx bake mcp-server
docker run --rm -e MCP_SERVER_ENABLED=true -p 8090:8090 lumendotapp/lumen-mcp-server:latest
curl -s localhost:8090/health   # {"status":"healthy","service":"mcp_server"}
```

### The Helm chart has never been rendered with `runtime: rust`

`helm` is not installed and its download is blocked by the egress proxy. The
template's control flow was checked for balance, and CI runs `ct lint` and
`helm lint`, but nobody has read the rendered output.

**Breaks as:** a malformed manifest rejected at apply time, or — worse — a
manifest that applies but drops the custom-CA volume, so the server cannot reach
the API over TLS and every request 401s.

```bash
helm template deployment/helm/charts/lumen \
  --set mcpServer.enabled=true --set mcpServer.runtime=rust \
  --set customCACerts.enabled=true --set customCACerts.secretName=my-ca \
  | sed -n '/kind: Deployment/,/^---/p'
```

Read for: the image is `lumen-mcp-server`, no `command:` overrides the
entrypoint, `LUMEN_CUSTOM_CA_CERTS_DIR` is present, and the `custom-ca-certs`
volume is mounted at `/etc/lumen/certs`.

### Custom CA certificates have never been exercised end to end

The loader is unit-tested against a real self-signed PEM, but no Rust client has
completed a TLS handshake against a server using a private CA.

**Breaks as:** every upstream call fails the handshake. Because auth delegates to
`GET /me`, that surfaces to clients as `401 Unauthorized`, which looks like a
credential problem and not a TLS one. The metric that tells them apart is
`lumen_mcp_server_auth_total{result="error"}` — transport failure — against
`{result="rejected"}`, which is a real bad token.

### `lumen_text` on malformed HTML

Measured, not unknown: **27% of deliberately malformed documents differ**. lxml
and html5ever recover from broken markup differently and no port can fix that.
Well-formed HTML is byte-identical.

**Breaks as:** indexed text changes for some crawled pages — different chunks,
different embeddings, quietly different search results. Nothing errors.

```bash
uv run python backend/native/parity/check_corpus.py files --path /some/crawl/dir
```

Run it against a real crawl sample before turning the flag on. That is the whole
reason the flag exists.

### The connection layer has never run against the production schema

It was tested against a throwaway cluster with hand-made schemas, not against a
real Lumen database with 447 migrations applied.

**Breaks as:** row decoding fails on a type the generator mapped wrongly. Loud
per query, not silent — the generator refuses unknown types rather than guessing,
which is what keeps this in the "errors" column instead of the "corruption" one.

### Sharding is refused outright

`Database::connect` errors when shard specs are configured, rather than routing
to the default.

**Breaks as:** a sharded deployment cannot start a Rust service at all. That is
the intended behaviour: the Python router fails closed because guessing
"default" for an already-migrated tenant sends its writes to the database it
moved off. Approximating it would be worse than refusing.

### The sandbox proxy is a CA bootstrap and nothing else yet

What exists: CA generation, validation, the file-backed store, and the atomic
materialization the proxy reads. What does not: the MITM engine, identity
resolution, credential injection, and the gate.

**Breaks as:** nothing. No binary runs it and no deployment references it.

The part worth knowing is that the CA it produces is interchangeable with the
Python one — proven by generating with each and loading with the other, then
comparing subject, issuer, self-signedness, basic constraints and criticality,
path length, every key-usage bit, and the subject key identifier. That is the
property a cutover depends on: a proxy that regenerated instead of loading would
orphan every sandbox trust store at once.

Still unverifiable here, and it is the larger half:

- **Docker identity resolution** needs a Docker daemon.
- **Kubernetes identity resolution** needs a cluster and its informer API.
- **The MITM engine** has no Rust equivalent of mitmproxy; it is a rewrite, and
  a security-critical one.

## The one thing that nearly went wrong

An integration test caught a **cross-tenant read**. SQLAlchemy carries the schema
as a per-connection execution option; `SET search_path` is session state that
outlives the borrow. A pooled connection handed back with a tenant's search_path
still set was being reused for catalog work, which then read that tenant's rows.

Fixed by scoping every borrow and `RESET`ting on unscoped ones. It is called out
here because it is the failure mode to look for in everything built on this
layer, and because unit tests would never have found it — only a real pool
against a real server did.

## Rolling back

| Component | Roll back by |
| --- | --- |
| `lumen_text` | Unset `LUMEN_NATIVE_TEXT` and restart. No rebuild needed. |
| MCP server | `mcpServer.runtime: python`. Same port, same health path, same metric names. |
| `lumen-db` | Nothing to roll back; no code path uses it yet. |

## Reproducing the test environment

No Docker needed — the PostgreSQL 16 server binaries are already installed:

```bash
initdb=/usr/lib/postgresql/16/bin
$initdb/initdb -D /var/lib/postgresql/lumen-test -U postgres --auth=trust
$initdb/pg_ctl -D /var/lib/postgresql/lumen-test -l /tmp/pg.log \
  -o "-p 5432 -c listen_addresses=127.0.0.1" start

psql -h 127.0.0.1 -U postgres -f backend/native/lumen-db/tests/fixtures/schemas.sql

LUMEN_DB_TEST=1 POSTGRES_HOST=127.0.0.1 POSTGRES_USER=postgres \
  POSTGRES_PASSWORD=password POSTGRES_DB=postgres \
  cargo test --manifest-path backend/native/Cargo.toml -p lumen-db
```

Without `LUMEN_DB_TEST`, those tests print a skip instead of failing — which
means **a CI run that does not set it proves nothing about tenant isolation.**
