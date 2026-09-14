# Lumen Prometheus Metrics Reference

## Adding New Metrics

All Prometheus metrics live in the `backend/lumen/server/metrics/` package. Follow these steps to add a new metric.

### 1. Choose the right file (or create a new one)

| File                                  | Purpose                                      |
| ------------------------------------- | -------------------------------------------- |
| `metrics/slow_requests.py`            | Slow request counter + callback              |
| `metrics/postgres_connection_pool.py` | SQLAlchemy connection pool metrics           |
| `metrics/prometheus_setup.py`         | FastAPI instrumentator config (orchestrator) |

If your metric is a standalone concern (e.g. cache hit rates, queue depths), create a new file under `metrics/` and keep one metric concept per file.

### 2. Define the metric

Use `prometheus_client` types directly at module level:

```python
# metrics/my_metric.py
from prometheus_client import Counter

_my_counter = Counter(
    "lumen_my_counter_total",  # Always prefix with lumen_
    "Human-readable description",
    ["label_a", "label_b"],  # Keep label cardinality low
)
```

**Naming conventions:**

- Prefix all metric names with `lumen_`
- Counters: `_total` suffix (e.g. `lumen_api_slow_requests_total`)
- Histograms: `_seconds` or `_bytes` suffix for durations/sizes
- Gauges: no special suffix

**Label cardinality:** Avoid high-cardinality labels (raw user IDs, UUIDs, raw paths). Use route templates like `/api/items/{item_id}` instead of `/api/items/abc-123`.

### 3. Wire it into the instrumentator (if request-scoped)

If your metric needs to run on every HTTP request, write a callback and register it in `prometheus_setup.py`:

```python
# metrics/my_metric.py
from prometheus_fastapi_instrumentator.metrics import Info


def my_metric_callback(info: Info) -> None:
    _my_counter.labels(label_a=info.method, label_b=info.modified_handler).inc()
```

```python
# metrics/prometheus_setup.py
from lumen.server.metrics.my_metric import my_metric_callback

# Inside setup_prometheus_metrics():
instrumentator.add(my_metric_callback)
```

### 4. Register infrastructure metrics after resource initialization

For metrics that attach to engines, pools, or background systems, add a setup
function and call it from the owning process lifespan after the resource exists:

```python
# metrics/my_metric.py
def setup_my_metrics(resource: SomeResource) -> None:
    # Register collectors, attach event listeners, etc.
    ...
```

```python
# lumen/main.py — lifespan, after resource initialization
from lumen.server.metrics.my_metric import setup_my_metrics

setup_my_metrics(resource)
```

Keep `setup_prometheus_metrics()` limited to shared HTTP instrumentation.

### 5. Write tests

Add tests in `backend/tests/unit/lumen/server/`. Use `unittest.mock.patch` to mock the prometheus objects — don't increment real global counters in tests.

### 6. Document the metric

Add your metric to the reference tables below in this file. Include the metric name, type, labels, and description.

### 7. Update Grafana dashboards

After deploying, add panels to the relevant Grafana dashboard:

1. Open Grafana and navigate to the Lumen dashboard (or create a new one)
2. Add a new panel — choose the appropriate visualization:
   - **Counters** → use `rate()` in a time series panel (e.g. `rate(lumen_my_counter_total[5m])`)
   - **Histograms** → use `histogram_quantile()` for percentiles, or `_sum/_count` for averages
   - **Gauges** → display directly as a stat or gauge panel
3. Add meaningful thresholds and alerts where appropriate
4. Group related panels into rows (e.g. "API Performance", "Database Pool")

---

## API Server Metrics

These metrics are exposed at `GET /metrics` on the API server.

### Built-in (via `prometheus-fastapi-instrumentator`)

| Metric                                | Type      | Labels                        | Description                                       |
| ------------------------------------- | --------- | ----------------------------- | ------------------------------------------------- |
| `http_requests_total`                 | Counter   | `method`, `status`, `handler` | Total request count                               |
| `http_request_duration_highr_seconds` | Histogram | _(none)_                      | High-resolution latency (many buckets, no labels) |
| `http_request_duration_seconds`       | Histogram | `method`, `handler`           | Latency by handler (custom buckets for P95/P99)   |
| `http_request_size_bytes`             | Summary   | `handler`                     | Incoming request content length                   |
| `http_response_size_bytes`            | Summary   | `handler`                     | Outgoing response content length                  |
| `http_requests_inprogress`            | Gauge     | `method`, `handler`           | Currently in-flight requests                      |

### Custom (via `lumen.server.metrics`)

| Metric                         | Type    | Labels                        | Description                                                      |
| ------------------------------ | ------- | ----------------------------- | ---------------------------------------------------------------- |
| `lumen_api_slow_requests_total` | Counter | `method`, `handler`, `status` | Requests exceeding `SLOW_REQUEST_THRESHOLD_SECONDS` (default 1s) |

### Configuration

| Env Var                          | Default | Description                                  |
| -------------------------------- | ------- | -------------------------------------------- |
| `SLOW_REQUEST_THRESHOLD_SECONDS` | `1.0`   | Duration threshold for slow request counting |

The API and MCP `/metrics` endpoints require `Authorization: Bearer
<METRICS_AUTH_TOKEN>`. They fail closed when no token is configured; set
`DISABLE_METRICS_AUTH=true` only when unauthenticated access is intentional.
The Helm API and MCP ServiceMonitors automatically reference
`auth.metricsAuth` when it is enabled.

### Instrumentator Settings

- `should_group_status_codes=False` — Reports exact HTTP status codes (e.g. 401, 403, 500)
- `should_instrument_requests_inprogress=True` — Enables the in-progress request gauge
- `inprogress_labels=True` — Breaks down in-progress gauge by `method` and `handler`
- `excluded_handlers=["/health", "/metrics", "/openapi.json"]` — Excludes noisy endpoints from metrics

## Connector State Metrics

The API server collects these metrics from Postgres on each scrape. They are
available only in single-tenant deployments; multi-tenant collection is skipped
to avoid cross-tenant data exposure.

| Metric                                                    | Type  | Labels                                         | Description                                                   |
| --------------------------------------------------------- | ----- | ---------------------------------------------- | ------------------------------------------------------------- |
| `lumen_connector_state_collection_success`                 | Gauge | _(none)_                                       | Whether the latest bounded snapshot read succeeded            |
| `lumen_connector_last_successful_index_timestamp_seconds`  | Gauge | `source`, `cc_pair_id`                         | Last successful index timestamp; zero means never              |
| `lumen_connector_last_pruned_timestamp_seconds`            | Gauge | `source`, `cc_pair_id`                         | Last successful prune timestamp; zero means never              |
| `lumen_connector_last_perm_sync_timestamp_seconds`         | Gauge | `source`, `cc_pair_id`                         | Last permission sync timestamp; zero means never               |
| `lumen_connector_last_external_group_sync_timestamp_seconds` | Gauge | `source`, `cc_pair_id`                       | Last external-group sync timestamp; zero means never           |
| `lumen_connector_repeated_error_state`                     | Gauge | `source`, `cc_pair_id`                         | Whether the connector is in a repeated error state             |
| `lumen_connector_status`                                   | Gauge | `source`, `cc_pair_id`, `status`               | One-hot current connector status                               |
| `lumen_connector_access_type`                              | Gauge | `source`, `cc_pair_id`, `access_type`          | One-hot connector access type                                  |
| `lumen_connector_indexing_trigger`                         | Gauge | `source`, `cc_pair_id`, `trigger_mode`         | One-hot indexing trigger; includes `NONE` and `UNKNOWN`         |
| `lumen_connector_auto_sync_enabled`                        | Gauge | `source`, `cc_pair_id`                         | Whether auto-sync is configured                                |
| `lumen_connector_count`                                    | Gauge | `source`, `status`                             | Current connector count                                        |
| `lumen_connector_document_count`                           | Gauge | `source`                                       | Current indexed document count                                 |
| `lumen_connector_info`                                     | Info  | `cc_pair_id`, connector metadata               | Display name, source, credential ID, status, and access type   |

The collector stops waiting after eight seconds and permits one in-flight read.
It does not return stale connector samples after a timeout or error; use
`lumen_connector_state_collection_success` to alert on missing snapshots.
Display names appear only in the info metric, limiting rename churn to one
series per connector.

## MCP Metrics

The MCP server exposes its HTTP and custom metrics on its authenticated
`/metrics` endpoint. MCP client metrics are exposed by the API server because
that process calls external MCP servers.

| Metric                                  | Process    | Type      | Labels                               | Description                                      |
| --------------------------------------- | ---------- | --------- | ------------------------------------ | ------------------------------------------------ |
| `lumen_mcp_server_auth_total`            | MCP server | Counter   | `result`                             | API token verification outcomes                  |
| `lumen_mcp_server_tool_calls_total`      | MCP server | Counter   | `tool`, `status`                     | Search-tool execution outcomes                    |
| `lumen_mcp_server_tool_latency_seconds`  | MCP server | Histogram | `tool`                               | Search-tool execution latency                     |
| `lumen_mcp_server_search_results`        | MCP server | Histogram | `tool`                               | Results returned by search tools                  |
| `lumen_mcp_server_search_by_source_total`| MCP server | Counter   | `source_type`                        | Searches requesting each deduplicated source      |
| `lumen_mcp_client_tool_calls_total`      | API server | Counter   | `server_name`, `tool_name`, `status` | Calls from Lumen to external MCP tools             |
| `lumen_mcp_client_tool_latency_seconds`  | API server | Histogram | `server_name`, `tool_name`           | External MCP tool latency                         |

Server tool statuses describe execution reliability. An empty tenant is a
successful search with zero results. `lumen_mcp_server_auth_total` records token
verifier outcomes; use HTTP request metrics as the source of truth for all 401s,
including requests rejected before verification. Client `server_name` is the
configured display name. `tool_name` is the execution name and may be sanitized
or disambiguated when tools collide.

## Database Pool Metrics

These metrics provide visibility into SQLAlchemy connection pool state across all three engines (`sync`, `async`, `readonly`). Collected via `lumen.server.metrics.postgres_connection_pool`.

### Pool State (via custom Prometheus collector — snapshot on each scrape)

| Metric                     | Type  | Labels   | Description                                     |
| -------------------------- | ----- | -------- | ----------------------------------------------- |
| `lumen_db_pool_checked_out` | Gauge | `engine` | Currently checked-out connections               |
| `lumen_db_pool_checked_in`  | Gauge | `engine` | Idle connections available in the pool          |
| `lumen_db_pool_overflow`    | Gauge | `engine` | Current overflow connections beyond `pool_size` |
| `lumen_db_pool_size`        | Gauge | `engine` | Configured pool size (constant)                 |

### Pool Lifecycle (via SQLAlchemy pool event listeners)

| Metric                                   | Type    | Labels   | Description                              |
| ---------------------------------------- | ------- | -------- | ---------------------------------------- |
| `lumen_db_pool_checkout_total`            | Counter | `engine` | Total connection checkouts from the pool |
| `lumen_db_pool_checkin_total`             | Counter | `engine` | Total connection checkins to the pool    |
| `lumen_db_pool_connections_created_total` | Counter | `engine` | Total new database connections created   |
| `lumen_db_pool_invalidations_total`       | Counter | `engine` | Total connection invalidations           |
| `lumen_db_pool_checkout_timeout_total`    | Counter | `engine` | Total connection checkout timeouts       |

### Per-Endpoint Attribution (via pool events + endpoint context middleware)

| Metric                                 | Type      | Labels              | Description                                     |
| -------------------------------------- | --------- | ------------------- | ----------------------------------------------- |
| `lumen_db_connections_held_by_endpoint` | Gauge     | `handler`, `engine` | DB connections currently held, by endpoint      |
| `lumen_db_connection_hold_seconds`      | Histogram | `handler`, `engine` | Duration a DB connection is held by an endpoint |

Engine label values: `sync` (main read-write), `async` (async sessions), `readonly` (read-only user).

Connections from background tasks (Celery) or boot-time warmup appear as `handler="unknown"`.

## Celery Worker Metrics

Celery workers expose metrics via a standalone Prometheus HTTP server (separate from the API server's `/metrics` endpoint). Each worker type runs its own server on a dedicated port.

### Metrics Server (`lumen.server.metrics.metrics_server`)

| Env Var                      | Default             | Description                                           |
| ---------------------------- | ------------------- | ----------------------------------------------------- |
| `PROMETHEUS_METRICS_PORT`    | _(per worker type)_ | Override the default port for this worker             |
| `PROMETHEUS_METRICS_ENABLED` | `true`              | Set to `false` to disable the metrics server entirely |

Default ports:

| Worker          | Port |
| --------------- | ---- |
| `docfetching`   | 9092 |
| `docprocessing` | 9093 |
| `monitoring`    | 9096 |

Workers without a default port and no `PROMETHEUS_METRICS_PORT` env var will skip starting the server.

### Generic Task Lifecycle Metrics (`lumen.server.metrics.celery_task_metrics`)

Push-based metrics that fire on Celery signals for all tasks on the worker.

| Metric                              | Type      | Labels                          | Description                                                                   |
| ----------------------------------- | --------- | ------------------------------- | ----------------------------------------------------------------------------- |
| `lumen_celery_task_started_total`    | Counter   | `task_name`, `queue`            | Total tasks started                                                           |
| `lumen_celery_task_completed_total`  | Counter   | `task_name`, `queue`, `outcome` | Total tasks completed (`outcome`: `success` or `failure`)                     |
| `lumen_celery_task_duration_seconds` | Histogram | `task_name`, `queue`            | Task execution duration. Buckets: 1, 5, 15, 30, 60, 120, 300, 600, 1800, 3600 |
| `lumen_celery_tasks_active`          | Gauge     | `task_name`, `queue`            | Currently executing tasks                                                     |
| `lumen_celery_task_retried_total`    | Counter   | `task_name`, `queue`            | Total task retries                                                            |
| `lumen_celery_task_revoked_total`    | Counter   | `task_name`                     | Total tasks revoked (cancelled)                                               |
| `lumen_celery_task_rejected_total`   | Counter   | `task_name`                     | Total tasks rejected by worker                                                |

Stale start-time entries (tasks killed via SIGTERM/OOM where `task_postrun` never fires) are evicted after 1 hour.

### Per-Connector Indexing Metrics (`lumen.server.metrics.indexing_task_metrics`)

Enriches docfetching and docprocessing tasks with connector-level labels. Silently no-ops for all other tasks.

| Metric                                | Type      | Labels                                                      | Description                              |
| ------------------------------------- | --------- | ----------------------------------------------------------- | ---------------------------------------- |
| `lumen_indexing_task_started_total`    | Counter   | `task_name`, `source`, `tenant_id`, `cc_pair_id`            | Indexing tasks started per connector     |
| `lumen_indexing_task_completed_total`  | Counter   | `task_name`, `source`, `tenant_id`, `cc_pair_id`, `outcome` | Indexing tasks completed per connector   |
| `lumen_indexing_task_duration_seconds` | Histogram | `task_name`, `source`, `tenant_id`                          | Indexing task duration by connector type |

`connector_name` is intentionally excluded from these per-task counters to avoid unbounded cardinality (it's a free-form user string).

### Connector Health Metrics (`lumen.server.metrics.connector_health_metrics`)

Push-based metrics emitted by docfetching and docprocessing workers at the point where connector state changes occur. Scales to any number of tenants (no schema iteration). Unlike the per-task counters above, these include `connector_name` because their cardinality is bounded by the number of connectors (one series per connector), not by the number of task executions.

| Metric                                          | Type    | Labels                                                          | Description                                                   |
| ----------------------------------------------- | ------- | --------------------------------------------------------------- | ------------------------------------------------------------- |
| `lumen_index_attempt_transitions_total`          | Counter | `tenant_id`, `source`, `cc_pair_id`, `connector_name`, `status` | Index attempt status transitions (in_progress, success, etc.) |
| `lumen_connector_in_error_state`                 | Gauge   | `tenant_id`, `source`, `cc_pair_id`, `connector_name`           | Whether connector is in repeated error state (1=yes, 0=no)    |
| `lumen_connector_last_success_timestamp_seconds` | Gauge   | `tenant_id`, `source`, `cc_pair_id`, `connector_name`           | Unix timestamp of last successful indexing                    |
| `lumen_connector_docs_indexed_total`             | Counter | `tenant_id`, `source`, `cc_pair_id`, `connector_name`           | Total documents indexed per connector (monotonic)             |
| `lumen_connector_indexing_errors_total`          | Counter | `tenant_id`, `source`, `cc_pair_id`, `connector_name`           | Total failed index attempts per connector (monotonic)         |

### Pull-Based Collectors (`lumen.server.metrics.indexing_pipeline`)

Registered only in the **Monitoring** worker. Collectors query Redis at scrape time with a 30-second TTL cache and a 120-second timeout to prevent the `/metrics` endpoint from hanging.

| Metric                               | Type  | Labels  | Description                         |
| ------------------------------------ | ----- | ------- | ----------------------------------- |
| `lumen_queue_depth`                   | Gauge | `queue` | Celery queue length                 |
| `lumen_queue_unacked`                 | Gauge | `queue` | Unacknowledged messages per queue   |
| `lumen_queue_oldest_task_age_seconds` | Gauge | `queue` | Age of the oldest task in the queue |

### Adding Metrics to a Worker

Currently only the docfetching and docprocessing workers have push-based task metrics wired up. To add metrics to another worker (e.g. heavy, light, primary):

**1. Import and call the generic handlers from the worker's signal handlers:**

```python
from lumen.server.metrics.celery_task_metrics import (
    on_celery_task_prerun,
    on_celery_task_postrun,
    on_celery_task_retry,
    on_celery_task_revoked,
    on_celery_task_rejected,
)


@signals.task_prerun.connect
def on_task_prerun(sender, task_id, task, args, kwargs, **kwds):
    app_base.on_task_prerun(sender, task_id, task, args, kwargs, **kwds)
    on_celery_task_prerun(task_id, task)
```

Do the same for `task_postrun`, `task_retry`, `task_revoked`, and `task_rejected` — see `apps/docfetching.py` for the complete example.

**2. Start the metrics server on `worker_ready`:**

```python
from lumen.server.metrics.metrics_server import start_metrics_server


@worker_ready.connect
def on_worker_ready(sender, **kwargs):
    start_metrics_server("your_worker_type")
    app_base.on_worker_ready(sender, **kwargs)
```

Add a default port for your worker type in `metrics_server.py`'s `_DEFAULT_PORTS` dict, or set `PROMETHEUS_METRICS_PORT` in the environment.

**3. (Optional) Add domain-specific enrichment:**

If your tasks need richer labels beyond `task_name`/`queue`, create a new module in `server/metrics/` following `indexing_task_metrics.py`:

- Define Counters/Histograms with your domain labels
- Write `on_<domain>_task_prerun` / `on_<domain>_task_postrun` handlers that filter by task name and no-op for others
- Call them from the worker's signal handlers alongside the generic ones

**Cardinality warning:** Never use user-defined free-form strings as metric labels — they create unbounded cardinality. Use IDs or enum values. If you need free-form labels, use pull-based collectors (monitoring worker) where cardinality is naturally bounded.

### Current Worker Integration Status

| Worker               | Generic Task Metrics | Domain Metrics | Metrics Server                       |
| -------------------- | -------------------- | -------------- | ------------------------------------ |
| Docfetching          | ✓                    | ✓ (indexing)   | ✓ (port 9092)                        |
| Docprocessing        | ✓                    | ✓ (indexing)   | ✓ (port 9093)                        |
| Monitoring           | —                    | —              | ✓ (port 9096, pull-based collectors) |
| Primary              | —                    | —              | —                                    |
| Light                | —                    | —              | —                                    |
| Heavy                | —                    | —              | —                                    |
| User File Processing | —                    | —              | —                                    |
| KG Processing        | —                    | —              | —                                    |

### Example PromQL Queries (Celery)

```promql
# Task completion rate by worker queue
sum by (queue) (rate(lumen_celery_task_completed_total[5m]))

# P95 task duration for pruning tasks
histogram_quantile(0.95,
  sum by (le) (rate(lumen_celery_task_duration_seconds_bucket{task_name=~".*pruning.*"}[5m])))

# Task failure rate
sum by (task_name) (rate(lumen_celery_task_completed_total{outcome="failure"}[5m]))
  / sum by (task_name) (rate(lumen_celery_task_completed_total[5m]))

# Active tasks per queue
sum by (queue) (lumen_celery_tasks_active)

# Indexing throughput by source type
sum by (source) (rate(lumen_indexing_task_completed_total{outcome="success"}[5m]))

# Queue depth — are tasks backing up?
lumen_queue_depth > 100
```

## OpenSearch Search Metrics

These metrics track OpenSearch search latency and throughput. Collected via `lumen.server.metrics.opensearch_search`.

| Metric                                           | Type      | Labels        | Description                                                                 |
| ------------------------------------------------ | --------- | ------------- | --------------------------------------------------------------------------- |
| `lumen_opensearch_search_client_duration_seconds` | Histogram | `search_type` | Client-side end-to-end latency (network + serialization + server execution) |
| `lumen_opensearch_search_server_duration_seconds` | Histogram | `search_type` | Server-side execution time from OpenSearch `took` field                     |
| `lumen_opensearch_search_total`                   | Counter   | `search_type` | Total search requests sent to OpenSearch                                    |
| `lumen_opensearch_searches_in_progress`           | Gauge     | `search_type` | Currently in-flight OpenSearch searches                                     |

Search type label values: See `OpenSearchSearchType`.

---

## Example PromQL Queries

### Which endpoints are saturated right now?

```promql
# Top 10 endpoints by in-progress requests
topk(10, http_requests_inprogress)
```

### What's the P99 latency per endpoint?

```promql
# P99 latency by handler over the last 5 minutes
histogram_quantile(0.99, sum by (handler, le) (rate(http_request_duration_seconds_bucket[5m])))
```

### Which endpoints have the highest request rate?

```promql
# Requests per second by handler, top 10
topk(10, sum by (handler) (rate(http_requests_total[5m])))
```

### Which endpoints are returning errors?

```promql
# 5xx error rate by handler
sum by (handler) (rate(http_requests_total{status=~"5.."}[5m]))
```

### Slow request hotspots

```promql
# Slow requests per minute by handler
sum by (handler) (rate(lumen_api_slow_requests_total[5m])) * 60
```

### Latency trending up?

```promql
# Compare P50 latency now vs 1 hour ago
histogram_quantile(0.5, sum by (le) (rate(http_request_duration_highr_seconds_bucket[5m])))
  -
histogram_quantile(0.5, sum by (le) (rate(http_request_duration_highr_seconds_bucket[5m] offset 1h)))
```

### Overall request throughput

```promql
# Total requests per second across all endpoints
sum(rate(http_requests_total[5m]))
```

### Pool utilization (% of capacity in use)

```promql
# Sync pool utilization: checked-out / (pool_size + max_overflow)
# NOTE: Replace 10 with your actual POSTGRES_API_SERVER_POOL_OVERFLOW value.
lumen_db_pool_checked_out{engine="sync"} / (lumen_db_pool_size{engine="sync"} + 10) * 100
```

### Pool approaching exhaustion?

```promql
# Alert when checked-out connections exceed 80% of pool capacity
# NOTE: Replace 10 with your actual POSTGRES_API_SERVER_POOL_OVERFLOW value.
lumen_db_pool_checked_out{engine="sync"} > 0.8 * (lumen_db_pool_size{engine="sync"} + 10)
```

### Which endpoints are hogging DB connections?

```promql
# Top 10 endpoints by connections currently held
topk(10, lumen_db_connections_held_by_endpoint{engine="sync"})
```

### Which endpoints hold connections the longest?

```promql
# P99 connection hold time by endpoint
histogram_quantile(0.99, sum by (handler, le) (rate(lumen_db_connection_hold_seconds_bucket{engine="sync"}[5m])))
```

### Connection checkout/checkin rate

```promql
# Checkouts per second by engine
sum by (engine) (rate(lumen_db_pool_checkout_total[5m]))
```

### Connector snapshot health and staleness

```promql
# Snapshot read failed or timed out
lumen_connector_state_collection_success == 0

# No successful indexing in 24 hours; zero ("never") also alerts
time() - lumen_connector_last_successful_index_timestamp_seconds > 86400

# Connectors currently stuck in repeated errors
lumen_connector_repeated_error_state == 1
```

### MCP tool failures

```promql
# MCP server tool execution failure rate
sum by (tool) (rate(lumen_mcp_server_tool_calls_total{status="error"}[5m]))
  / sum by (tool) (rate(lumen_mcp_server_tool_calls_total[5m]))

# External MCP client failures, including authentication errors
sum by (server_name, tool_name) (rate(lumen_mcp_client_tool_calls_total{status!="success"}[5m]))
```

### OpenSearch P99 search latency by type

```promql
# P99 client-side latency by search type
histogram_quantile(0.99, sum by (search_type, le) (rate(lumen_opensearch_search_client_duration_seconds_bucket[5m])))
```

### OpenSearch search throughput

```promql
# Searches per second by type
sum by (search_type) (rate(lumen_opensearch_search_total[5m]))
```

### OpenSearch concurrent searches

```promql
# Total in-flight searches across all instances
sum(lumen_opensearch_searches_in_progress)
```

### OpenSearch network overhead

```promql
# Difference between client and server P50 reveals network/serialization cost.
histogram_quantile(0.5, sum by (le) (rate(lumen_opensearch_search_client_duration_seconds_bucket[5m])))
  -
histogram_quantile(0.5, sum by (le) (rate(lumen_opensearch_search_server_duration_seconds_bucket[5m])))
```
