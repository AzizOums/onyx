# Guide technique — Implémentation, déploiement & opérations

> Build **FLOSS** d'Onyx : Community Edition pur, 100 % open source, rebuildable
> intégralement depuis ce dépôt (aucun binaire fermé, aucune licence requise).
> Public : équipe technique / DevOps.

---

## 1. Architecture

### Vue d'ensemble

```
                        ┌─────────────┐
        Utilisateurs ──▶│    nginx    │◀── TLS (80/443)
                        └──────┬──────┘
                ┌──────────────┴──────────────┐
                ▼                             ▼
        ┌──────────────┐              ┌──────────────┐
        │  web_server  │              │  api_server  │
        │  (Next.js)   │───/api/* ───▶│  (FastAPI)   │
        └──────────────┘              └──────┬───────┘
                                             │
             ┌───────────────┬───────────────┼──────────────────┐
             ▼               ▼               ▼                  ▼
      ┌────────────┐  ┌────────────┐  ┌──────────────┐  ┌──────────────┐
      │ relational │  │ opensearch │  │    cache     │  │ minio (S3)   │
      │ DB Postgres│  │  (index    │  │    Redis     │  │  fichiers    │
      └────────────┘  │  vectoriel)│  └──────────────┘  └──────────────┘
                      └────────────┘
                                             │
                             ┌───────────────┴────────────────┐
                             ▼                                ▼
                     ┌──────────────┐                ┌──────────────┐
                     │  background  │                │ model servers│
                     │ (Celery x9)  │                │  (embeddings │
                     └──────────────┘                │  + rerank)   │
                                                     └──────────────┘
```

### Briques et images

| Service | Image | Build depuis les sources | Rôle |
|---|---|---|---|
| `api_server` | `onyxdotapp/onyx-backend` | ✅ `backend/Dockerfile` | API FastAPI (auth, chat, agents, admin) |
| `background` | idem backend | ✅ | Workers Celery : connecteurs, indexation, permissions, monitoring |
| `web_server` | `onyxdotapp/onyx-web-server` | ✅ `web/Dockerfile` | Frontend Next.js 16 / React 19 |
| `inference_model_server` | `onyxdotapp/onyx-model-server` | ✅ `backend/Dockerfile.model_server` | Embeddings query + rerank |
| `indexing_model_server` | idem | ✅ | Embeddings d'indexation |
| `onyx-sandbox` (Craft) | `onyxdotapp/sandbox` | ✅ `backend/onyx/server/features/build/sandbox/image/Dockerfile` | Sandboxes agents (OpenCode) |
| `relational_db` | `postgres:15.2-alpine` | tiers | Données relationnelles |
| `opensearch` | `opensearchproject/opensearch:3.6` | tiers | Index keyword + vectoriel |
| `cache` | `redis:7.4-alpine` | tiers | Broker Celery + cache |
| `minio` | `minio/minio` | tiers | Stockage fichiers (S3-compatible) |
| `nginx` | `nginx:1.25` | tiers | Reverse proxy, TLS |

**Spécificités du build FLOSS** (branche `floss`) :
- Code Enterprise Edition supprimé (`backend/ee/`, `web/src/ee/`) ; les features
  essentielles ont été **portées en CE** : gestion des groupes utilisateurs,
  partage d'agents par groupe, synchronisation des groupes, settings
  entreprise (branding/logo).
- Service `code-interpreter` supprimé (binaire sans source) — l'outil
  d'exécution Python des agents n'existe plus.
- `LICENSE_ENFORCEMENT_ENABLED=false` et `ee_features_enabled=True` côté
  backend : le front active les features portées, sans licence.
- Auth : email/mot de passe natif + SSO SAML/OIDC (implémentations CE).

### Chat multimodal & catalogue models.dev (feature ajoutée)

**Principe** : les capacités d'entrée des modèles (texte/image/audio/vidéo/PDF)
viennent de **[models.dev](https://models.dev)** (catalogue public, 213+
providers) au lieu d'être codées en dur :
- `backend/onyx/llm/modelsdev.py` : client + cache Redis 24 h
  (`DISABLE_MODELSDEV=true` pour couper) — lookup avec normalisation des noms.
- Flows `AUDIO_INPUT` / `VIDEO_INPUT` (`LLMModelFlowType`), persistés comme
  `VISION` dans `llm_model_flow` et exposés dans les vues/entités modèles.
- Endpoints admin : `GET /admin/llm/modelsdev/providers` (catalogue :
  endpoint API, clés d'auth, docs) et
  `GET /admin/llm/modelsdev/providers/{id}/models` (modalités, limites).
- La discovery openai-compatible est **enrichie** par models.dev
  (modalités + `max_input_tokens`).
- Upload : nouveaux `ChatFileType.AUDIO` / `VIDEO` (extensions autorisées,
  non indexés) ; le LLM reçoit des parts `input_audio` (base64) / vidéo
  (data-URL) quand le modèle le supporte, sinon un marqueur texte.
- Frontend : sélecteur de fichiers `accept` adaptatif au modèle courant +
  toasts par modalité ; browser models.dev dans les modales de providers
  (OpenAI-Compatible et Custom) ; option « Multimodal » par modèle.
- Gateways sans clé : le client LiteLLM envoie un bearer vide (et non
  `"not-needed"`, rejeté en 401 par les gateways strictes).
- Notifications de versions upstream désactivées par défaut
  (`FETCH_UPSTREAM_CHANGELOG=true` pour réactiver) ; les entrées admin
  sans backend sont masquées de la sidebar (pas d'upsell mort).

---

## 2. Lancer en local (Docker Compose)

### Prérequis
- Docker Desktop (macOS/Windows) ou Docker Engine + Compose v2 (Linux)
- 16 Go RAM minimum recommandé (OpenSearch + model servers sont gourmands)

### Démarrage rapide

```bash
cd deployment/docker_compose

# 1. Fichier d'environnement (une fois)
cp env.template .env

# 2. Secrets obligatoires à éditer dans .env :
#    USER_AUTH_SECRET="openssl rand -hex 32"   # requis sinon l'API refuse de démarrer
#    ENCRYPTION_KEY_SECRET=                     # recommandé (chiffrement des creds connecteurs)
#    POSTGRES_PASSWORD=                         # au premier boot uniquement
#    OPENSEARCH_ADMIN_PASSWORD=                 # au premier boot uniquement
#    LICENSE_ENFORCEMENT_ENABLED=false          # build FLOSS : obligatoire
#    INTEGRATION_TESTS_MODE=true                # si e2e (mock LLM)
#    MCP_SERVER_ALLOW_LOOPBACK=true             # si e2e MCP (mocks sur 127.0.0.1)

# 3. Lancer
docker compose up -d --wait
```

→ **http://localhost:3000** — le premier utilisateur inscrit devient admin.

### Commandes utiles

```bash
docker compose ps                        # état des conteneurs
docker compose logs -f api_server        # logs d'un service
docker compose logs background | grep -i error
docker compose restart nginx             # si 502 après recréation de l'API
docker compose down                      # stop (garde les volumes)
docker compose down -v                   # stop + efface TOUTES les données
docker compose build api_server web_server   # rebuild depuis les sources
docker compose up -d --force-recreate api_server  # appliquer un changement de .env
```

> ⚠️ Après `down -v`, les mots de passe Postgres/OpenSearch sont réappliqués
> (premier boot). Après un `down` simple, ils ne changent plus.

### Développement (code source, hot reload)

Prérequis : Python 3.13 + `uv`, Bun, Docker.

```bash
# Dépendances externes seules
cd deployment/docker_compose
docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d \
  index relational_db cache minio

# Backend (dans chaque terminal)
cd backend && uv sync
uv run alembic upgrade head
uv run uvicorn model_server.main:app --reload --port 9000   # model server
python ./scripts/dev_run_background_jobs.py                  # workers
AUTH_TYPE=basic uv run uvicorn onyx.main:app --reload --port 8080

# Frontend
cd web && bun install && bun run dev
```

Alternative recommandée : le debuggeur VSCode (`.vscode/launch.json`,
« Run All Onyx Services »), voir CONTRIBUTING.md.

---

## 3. Variables d'environnement clés

Référence complète : `deployment/docker_compose/env.template`.

| Variable | Rôle | Défaut |
|---|---|---|
| `IMAGE_TAG` | Version des images (`latest` = main nightly) | `latest` |
| `USER_AUTH_SECRET` | Signature des cookies (obligatoire) | — |
| `ENCRYPTION_KEY_SECRET` | Chiffrement des credentials connecteurs | — |
| `POSTGRES_HOST` / `POSTGRES_PASSWORD` | DB | `relational_db` / — |
| `OPENSEARCH_HOST` / `OPENSEARCH_ADMIN_PASSWORD` | Index | `opensearch` / — |
| `OPENSEARCH_USE_SSL` | TLS vers OpenSearch (bundled = HTTPS) | `true` |
| `REDIS_HOST` | Broker/cache | `cache` |
| `S3_ENDPOINT_URL` / `S3_AWS_*` | Stockage fichiers (MinIO ou S3) | `http://minio:9000` |
| `FILE_STORE_BACKEND` | `s3` \| `postgres` \| `gcs` \| `azure` | `s3` |
| `GEN_AI_API_KEY` | Clé LLM par défaut (configurable aussi via UI admin) | — |
| `SMTP_*` / `EMAIL_FROM` | Emails (invitations, reset password) | — |
| `ENABLE_CRAFT` | Sandboxes agents (Craft) | `false` |
| `SANDBOX_BACKEND` | `docker` (compose) \| `kubernetes` (helm) | `kubernetes` |
| `LICENSE_ENFORCEMENT_ENABLED` | **FLOSS : `false` obligatoire** | `true` |
| `INTEGRATION_TESTS_MODE` | Mock LLM pour e2e uniquement | — |
| `MCP_SERVER_ALLOW_LOOPBACK` | Permet aux MCP d'atteindre le loopback (mocks e2e) | `false` |
| `DISABLE_MODELSDEV` | Coupe les lookups models.dev (`true` = off) | — |
| `FETCH_UPSTREAM_CHANGELOG` | Notifs de versions upstream (`true` = on) | — |
| `LOG_LEVEL` | Verbosité globale | `info` |

---

## 4. Déploiement entreprise

### 4.1 Docker Compose « prod » (VM unique — jusqu'à ~100 utilisateurs)

Adapté à une VM 16–32 vCPU / 64–128 Go RAM / SSD 200 Go+.

```bash
cd deployment/docker_compose
cp env.template .env
# Éditer .env : mots de passe forts, secrets, DOMAIN
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d
```

L'overlay `docker-compose.prod.yml` :
- refuse les creds par défaut (`S3_AWS_*` obligatoires),
- route le trafic via nginx avec la config `app.conf.template.prod`,
- active certbot (Let's Encrypt) — montages `../data/certbot`.

Checklist sécurité prod :
1. N'exposer **que** nginx (80/443) — retirer les ports des autres services.
2. `docker compose down -v` jamais en prod ; backups reguliers (§5).
3. HTTPS obligatoire (certbot ou LB) — définir `DOMAIN` + `WEB_DOMAIN`.
4. SSO via Admin Panel > Organization > SSO Providers (SAML/OIDC).
5. Secrets dans un gestionnaire (Vault, AWS Secrets Manager).

### 4.2 Kubernetes (Helm) — AWS / GCP / Azure / on-prem

```bash
helm repo add onyx https://onyx-dot-app.github.io/onyx
helm repo update
helm install onyx onyx/onyx -n onyx --create-namespace \
  --set auth.opensearch.values.opensearch_admin_password='...'
```

Le chart inclut : Postgres (CloudNativePG), Redis (redis-operator), MinIO,
autoscaling (HPA/KEDA), ingress + Let's Encrypt, dashboards Grafana,
External Secrets Operator (Vault / AWS SM / GCP SM).

Recommandations cloud-managé (recommandé en prod) :

| Composant | AWS | GCP | Azure |
|---|---|---|---|
| Postgres | RDS/Aurora | Cloud SQL | Azure Database |
| Redis | ElastiCache | Memorystore | Azure Cache |
| Fichiers | S3 (`S3_ENDPOINT_URL=""`) | GCS (`FILE_STORE_BACKEND=gcs`) | Blob (`FILE_STORE_BACKEND=azure`) |
| OpenSearch | OpenSearch Service | — | — |

Exemple values (AWS managé) :

```yaml
postgresql:
  enabled: false
redis:
  enabled: false
minio:
  enabled: false
configMap:
  POSTGRES_HOST: mycluster.rds.amazonaws.com
  REDIS_HOST: my-elasticache-endpoint
  S3_FILE_STORE_BUCKET_NAME: my-bucket
```

Craft (agents avec sandbox) en K8s : `configMap.ENABLE_CRAFT=true`,
Kubernetes >= 1.33, image `onyxdotapp/sandbox` buildée depuis le repo
(`make craft-sandbox-image`).

### 4.3 GPU (optionnel)

Les model servers tournent sur CPU ; pour de gros volumes d'indexation,
décommenter le bloc `deploy.resources.reservations.devices` (nvidia) dans le
compose ou utiliser des node pools GPU en K8s.

---

## 5. Opérations

### Backups
- **Postgres** : `pg_dump` quotidien (tout l'état métier).
- **Volumes** : `db_volume`, `opensearch-data`, `minio_data` (ou backup natif
  du service managé).
- Restauration : restaurer la DB puis les volumes, `docker compose up -d`.

### Monitoring
- Endpoints Prometheus `/api/metrics` (Bearer `METRICS_AUTH_TOKEN`).
- Dashboards Grafana fournis dans `deployment/helm/charts/onyx/dashboards/`.
- Logs : `docker compose logs` ; rotation json-file déjà configurée.

### Mises à jour
```bash
# Compose
cd deployment/docker_compose
docker compose pull && docker compose up -d

# Helm
helm upgrade onyx onyx/onyx -n onyx
```
Compatibilité SemVer entre minor versions ; les migrations Alembic s'exécutent
au démarrage de l'API. Toujours tester une montée de version sur un staging.

### Dépannage rapide

| Symptôme | Cause probable | Fix |
|---|---|---|
| API ne démarre pas | `USER_AUTH_SECRET` vide | Générer `openssl rand -hex 32` |
| 502 via nginx après rebuild | cache DNS nginx | `docker compose restart nginx` |
| OpenSearch OOM-killed (exit 137) | Docker Desktop < 12 Go alloués | Monter la mémoire Docker, ou override local `OPENSEARCH_JAVA_OPTS=-Xms1g -Xmx1g` (non commité) |
| Workers crash-loop « OpenSearch readiness » | OpenSearch plus lent que le probe (60 s) | Relancer `background` après coup ; ajouter un healthcheck OpenSearch |
| Fichiers bloqués « Processing » | worker en crash-loop (voir ci-dessus) | Vérifier `docker compose logs background` |
| 401 gateway openai-compatible sans clé | bearer placeholder rejeté | Le backend envoie un bearer vide ; vérifier `api_key` nulle (pas de dummy) |
| Mots de passe DB ignorés | volumes existants | `down -v` (destruction !) puis `up` |

---

## 6. Tests

| Type | Commande | Prérequis |
|---|---|---|
| Unit | `uv run pytest backend/tests/unit` | aucun |
| Typecheck backend | `uv run ty check` | aucun |
| Typecheck web | `cd web && bun run types:check` | `bun install` |
| Jest (web) | `cd web && bunx jest` | `bun install` |
| E2E Playwright | `cd web && bunx playwright test --project admin` | stack docker up + Ollama |

E2E avec Ollama local :
1. Ollama lancé (`ollama serve`), modèle pullé (`ollama pull llama3.1:8b`).
2. Provider configuré via UI/API (`ollama_chat`, `http://host.docker.internal:11434`).
3. `.env` : `INTEGRATION_TESTS_MODE=true`, `MCP_SERVER_ALLOW_LOOPBACK=true`.
4. `PATH="$PWD/.venv/bin:$PATH"` pour les mocks MCP python.
5. Specs retirés/ajustés dans ce build : EE par design (SCIM, usage budgets,
   permission-sync, default-deny permissions) et Search UI payante.
6. Specs ajoutés : `multimodal_chat_input` (accept adaptatif + toast),
   `llm_providers_modelsdev` (création provider depuis le catalogue).

E2E multimodal (gateway gratuite OpenCode Zen, sans clé) :
- Provider openai-compatible `https://opencode.ai/zen/v1`, modèle
  `mimo-v2.5-free` (text/image/audio/video, quota gratuit limité).
- Vérifié : discovery enrichie, persistance des flags, vision via Onyx
  (carré rouge → « Red »), upload audio/vidéo acceptés. Si `FreeUsageLimitError`,
  attendre la fenêtre de quota.

Lint/format : `pre-commit run --files <paths>` (ruff, oxlint, oxfmt).

---

## 7. Structure du dépôt

```
backend/          API FastAPI (onyx/), workers Celery, migrations alembic/
  onyx/llm/modelsdev.py       client catalogue models.dev (cache Redis 24 h)
  onyx/server/user_group/     API gestion des groupes (portée EE→CE)
  onyx/server/enterprise_settings/  API settings/branding (portée EE→CE)
  onyx/db/user_group_crud.py  couche DB groupes (portée EE→CE)
web/              Frontend Next.js (src/), tests e2e (tests/e2e/)
deployment/       docker_compose/ (compose + templates) et helm/ (chart K8s)
cli/              onyx-cli (installer / gestion du cycle de vie)
docs/             Ce guide + guide utilisateur
desktop/ mobile/  Applications Tauri et Expo (builds séparés)
widget/           Widget chat embarquable
extensions/       Extension Chrome
tools/            ods (devtools), loadtest, profiling
```
