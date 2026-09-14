"""Unit tests for the models.dev catalog client."""

from lumen.llm.modelsdev import (
    _normalize_model_name,
    _parse_catalog,
    lookup_modelsdev_model,
)


def _sample_raw() -> dict:
    return {
        "opencode": {
            "name": "OpenCode Zen",
            "api": "https://opencode.ai/zen/v1",
            "env_keys": ["OPENCODE_API_KEY"],
            "doc": "https://opencode.ai/docs/zen",
            "models": {
                "mimo-v2.5-free": {
                    "name": "MiMo V2.5 Free",
                    "modalities": {
                        "input": ["text", "image", "audio", "video"],
                        "output": ["text"],
                    },
                    "limit": {"context": 200000, "output": 32000},
                    "reasoning": False,
                    "tool_call": True,
                }
            },
        },
        "openai": {
            "name": "OpenAI",
            "models": {
                "gpt-4o": {
                    "name": "GPT-4o",
                    "modalities": {"input": ["text", "image"], "output": ["text"]},
                    "limit": {"context": 128000},
                }
            },
        },
    }


def test_parse_catalog_modalities() -> None:
    catalog = _parse_catalog(_sample_raw())
    assert len(catalog) == 2
    mimo = catalog["opencode"].models["mimo-v2.5-free"]
    assert mimo.input_modalities == ["text", "image", "audio", "video"]
    assert mimo.supports_image_input is True
    assert mimo.supports_audio_input is True
    assert mimo.supports_video_input is True
    assert mimo.context_limit == 200000


def test_normalize_model_name() -> None:
    assert _normalize_model_name("xiaomi/mimo-v2.5") == "mimo-v2.5"
    assert _normalize_model_name("mimo-v2.5:free") == "mimo-v2.5"
    assert _normalize_model_name("GPT-4O") == "gpt-4o"


def test_lookup_cross_provider(monkeypatch) -> None:  # type: ignore[no-untyped-def]
    raw = _sample_raw()
    monkeypatch.setattr("lumen.llm.modelsdev.fetch_modelsdev_catalog", lambda: _parse_catalog(raw))
    # same model id under another provider name resolves via normalization
    found = lookup_modelsdev_model(None, "XIAOMI/MIMO-V2.5-FREE")
    assert found is not None
    assert found.supports_audio_input is True
    # exact provider+model hit
    found = lookup_modelsdev_model("opencode", "mimo-v2.5-free")
    assert found is not None and found.name == "MiMo V2.5 Free"
    # unknown model
    assert lookup_modelsdev_model(None, "does-not-exist-zzz") is None
