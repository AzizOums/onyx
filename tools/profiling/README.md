# Lumen Local Monitoring Stack

Prometheus + Grafana for local development. Pre-loaded with dashboards for the Lumen backend.

## Usage

```bash
cd tools/profiling/
docker compose up -d
```

| Service    | URL                          | Credentials   |
|------------|------------------------------|---------------|
| Grafana    | http://localhost:3001        | admin / admin |
| Prometheus | http://localhost:9090        | —             |

## Dashboards

- **Lumen DB Pool Health** — PostgreSQL connection pool utilization
- **Lumen Indexing Pipeline v2** — Per-connector indexing throughput, queue depth, task latency
- **Lumen Permission Sync** — Doc permission sync and external group sync duration, throughput, errors, and Celery task metrics

## Scrape targets

| Job                        | Port  | Source                        |
|----------------------------|-------|-------------------------------|
| `lumen-api-server`          | 8080  | FastAPI `/metrics` (matches `.vscode/launch.json`) |
| `lumen-monitoring-worker`   | 9096  | Celery monitoring worker      |
| `lumen-docfetching-worker`  | 9092  | Celery docfetching worker     |
| `lumen-docprocessing-worker`| 9093  | Celery docprocessing worker   |
| `lumen-heavy-worker`        | 9094  | Celery heavy worker (pruning, perm sync, group sync) |
| `lumen-light-worker`        | 9095  | Celery light worker (vespa sync, deletion, permissions upsert) |

## Environment variables

Override defaults with a `.env` file in this directory or by setting them in your shell:

| Variable            | Default | Description                     |
|---------------------|---------|---------------------------------|
| `PROMETHEUS_PORT`   | `9090`  | Host port for Prometheus UI     |
| `GRAFANA_PORT`      | `3001`  | Host port for Grafana UI        |
| `GF_ADMIN_PASSWORD` | `admin` | Grafana admin password          |

## Editing dashboards

`allowUiUpdates: true` is set in the provisioning config, so you can edit dashboards in the Grafana UI. However, **changes don't persist** across `docker compose down` — to keep edits, export the dashboard JSON and overwrite the file in `grafana/dashboards/lumen/`.
