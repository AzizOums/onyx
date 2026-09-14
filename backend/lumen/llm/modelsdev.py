"""Client for https://models.dev — the public LLM model catalog.

Provides provider discovery (endpoint URLs, auth env keys, docs) and per-model
capability data (input modalities: text/image/audio/video/pdf, context limits,
costs). Used by the admin LLM-provider flows to prefill provider configuration
and to know which file types a model can actually accept.
"""

import json
from functools import lru_cache
from typing import Any

import httpx
from pydantic import BaseModel

from lumen.utils.logger import setup_logger
from lumen.utils.variable_functionality import global_version

logger = setup_logger()

MODELSDEV_API_URL = "https://models.dev/api.json"
FETCH_TIMEOUT_S = 30.0
CACHE_TTL_S = 60 * 60 * 24  # 24h
REDIS_CACHE_KEY = "lumen:modelsdev:catalog"


class ModelsDevModel(BaseModel):
    id: str
    name: str
    input_modalities: list[str] = ["text"]
    output_modalities: list[str] = ["text"]
    context_limit: int | None = None
    max_output_tokens: int | None = None
    reasoning: bool = False
    tool_call: bool = False
    # Some entries nest tiered pricing (e.g. context_over_200k: {...}); keep raw.
    costs: dict[str, Any] = {}

    @property
    def supports_image_input(self) -> bool:
        return "image" in self.input_modalities

    @property
    def supports_audio_input(self) -> bool:
        return "audio" in self.input_modalities

    @property
    def supports_video_input(self) -> bool:
        return "video" in self.input_modalities

    @property
    def supports_pdf_input(self) -> bool:
        return "pdf" in self.input_modalities


class ModelsDevProvider(BaseModel):
    id: str
    name: str
    api: str | None = None
    doc: str | None = None
    env_keys: list[str] = []
    npm: str | None = None
    models: dict[str, ModelsDevModel] = {}


def _parse_model(provider_id: str, model_id: str, raw: dict[str, Any]) -> ModelsDevModel:
    modalities = raw.get("modalities") or {}
    limit = raw.get("limit") or {}
    return ModelsDevModel(
        id=model_id,
        name=raw.get("name") or model_id,
        input_modalities=modalities.get("input") or ["text"],
        output_modalities=modalities.get("output") or ["text"],
        context_limit=limit.get("context"),
        max_output_tokens=limit.get("output"),
        reasoning=bool(raw.get("reasoning")),
        tool_call=bool(raw.get("tool_call")),
        costs=raw.get("cost") or {},
    )


def _parse_catalog(raw: dict[str, Any]) -> dict[str, ModelsDevProvider]:
    providers: dict[str, ModelsDevProvider] = {}
    for provider_id, provider_raw in raw.items():
        models = {
            model_id: _parse_model(provider_id, model_id, model_raw)
            for model_id, model_raw in (provider_raw.get("models") or {}).items()
        }
        providers[provider_id] = ModelsDevProvider(
            id=provider_id,
            name=provider_raw.get("name") or provider_id,
            api=provider_raw.get("api"),
            doc=provider_raw.get("doc"),
            env_keys=provider_raw.get("env_keys")
            or provider_raw.get("env")
            or [],
            npm=provider_raw.get("npm"),
            models=models,
        )
    return providers


def fetch_modelsdev_catalog() -> dict[str, ModelsDevProvider]:
    """Fetch (and cache) the models.dev catalog.

    Redis cache in shared namespace when available; falls back to a direct
    fetch. Never raises for a cache hit; raises for network errors so callers
    can surface them to the admin.
    """
    from lumen.redis.redis_pool import get_redis_client

    redis_client = None
    try:
        redis_client = get_redis_client(tenant_id=None)
        cached = redis_client.get(REDIS_CACHE_KEY)
        if cached:
            return _parse_catalog(json.loads(cached))
    except Exception as e:  # noqa: BLE001 — cache is best-effort
        logger.debug("models.dev cache read failed: %s", e)

    response = httpx.get(MODELSDEV_API_URL, timeout=FETCH_TIMEOUT_S)
    response.raise_for_status()
    raw = response.json()
    catalog = _parse_catalog(raw)

    try:
        if redis_client is not None:
            redis_client.set(REDIS_CACHE_KEY, json.dumps(raw), ex=CACHE_TTL_S)
    except Exception as e:  # noqa: BLE001
        logger.debug("models.dev cache write failed: %s", e)

    return catalog


@lru_cache(maxsize=1)
def _normalization_strip_tokens() -> tuple[str, ...]:
    # Common vendor prefixes/suffixes that differ between providers
    return (
        "openrouter/",
        "xiaomi/",
        ":free",
        ":thinking",
    )


def _normalize_model_name(name: str) -> str:
    lowered = name.lower()
    for token in _normalization_strip_tokens():
        lowered = lowered.replace(token, "")
    return lowered.strip("/")


def lookup_modelsdev_model(
    provider_id: str | None, model_name: str
) -> ModelsDevModel | None:
    """Resolve a model in the catalog with progressive normalization.

    Tries: exact provider+model, normalized match within provider, then a
    global normalized match (same model served by another provider).
    """
    try:
        catalog = fetch_modelsdev_catalog()
    except Exception as e:  # noqa: BLE001 — capabilities are best-effort
        logger.warning("models.dev lookup failed: %s", e)
        return None

    normalized = _normalize_model_name(model_name)

    provider = catalog.get(provider_id) if provider_id else None
    if provider is not None:
        if model_name in provider.models:
            return provider.models[model_name]
        for model in provider.models.values():
            if _normalize_model_name(model.id) == normalized:
                return model

    # Global fallback: same model on any provider
    for candidate in catalog.values():
        for model in candidate.models.values():
            if _normalize_model_name(model.id) == normalized:
                return model

    return None


def modelsdev_enabled() -> bool:
    """Kill switch — upstream catalog lookups can be disabled per deployment."""
    import os

    return os.environ.get("DISABLE_MODELSDEV", "").lower() != "true"
