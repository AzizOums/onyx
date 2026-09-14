"""Unit tests for OpenCode Zen's two API-mode routing in LitellmLLM.

The surface is selected via an `opencode_api_mode` value persisted in
custom_config; the routing difference is the model= string, which drives
litellm's completions->responses bridge. Most free models speak chat
completions, but muse-spark contributor-free models are Responses-only
upstream (500 on /chat/completions, 200 on /responses). These tests lock
that mapping down.
"""

from unittest.mock import patch

from lumen.llm.api_surfaces import LlmApiSurface
from lumen.llm.constants import LlmProviderNames
from lumen.llm.custom_config_mapping import UI_ONLY_CONFIG_KEYS
from lumen.llm.models import LanguageModelInput, UserMessage
from lumen.llm.multi_llm import LitellmLLM
from lumen.llm.well_known_providers.constants import OPENCODE_API_MODE_CONFIG_KEY


def _make_opencode_llm(
    mode: str | None,
    api_base: str = "https://opencode.ai/zen/v1",
    model_name: str = "muse-spark-1.3-contributor-free",
) -> LitellmLLM:
    custom_config = {OPENCODE_API_MODE_CONFIG_KEY: mode} if mode is not None else None
    return LitellmLLM(
        api_key="public",
        timeout=30,
        model_provider=LlmProviderNames.OPENCODE,
        model_name=model_name,
        max_input_tokens=1_048_576,
        api_base=api_base,
        custom_config=custom_config,
    )


def _completion_kwargs(llm: LitellmLLM) -> dict:
    with patch("litellm.completion") as mock_completion:
        mock_completion.return_value = []
        messages: LanguageModelInput = [UserMessage(content="Hi")]
        list(llm.stream(messages))
        return dict(mock_completion.call_args.kwargs)


def test_chat_completions_mode_routes_via_openai_with_v1_base() -> None:
    llm = _make_opencode_llm("chat_completions")
    assert llm._custom_llm_provider == "openai"
    assert llm._api_base == "https://opencode.ai/zen/v1"

    kwargs = _completion_kwargs(llm)
    assert kwargs["custom_llm_provider"] == "openai"
    assert kwargs["base_url"] == "https://opencode.ai/zen/v1"
    # OpenAI-compatible proxies send a bare model name.
    assert kwargs["model"] == "muse-spark-1.3-contributor-free"


def test_default_mode_is_chat_completions() -> None:
    # No mode key (every pre-existing provider) -> old behavior preserved.
    llm = _make_opencode_llm(None)
    assert llm._api_surface is LlmApiSurface.OPENAI_CHAT_COMPLETIONS
    assert llm._custom_llm_provider == "openai"

    kwargs = _completion_kwargs(llm)
    assert kwargs["model"] == "muse-spark-1.3-contributor-free"


def test_unknown_mode_falls_back_to_chat_completions() -> None:
    llm = _make_opencode_llm("not-a-real-mode")
    assert llm._api_surface is LlmApiSurface.OPENAI_CHAT_COMPLETIONS


def test_responses_mode_prefixes_model_and_keeps_v1_base() -> None:
    llm = _make_opencode_llm("responses")
    assert llm._api_surface is LlmApiSurface.OPENAI_RESPONSES
    assert llm._custom_llm_provider == "openai"
    assert llm._api_base == "https://opencode.ai/zen/v1"

    kwargs = _completion_kwargs(llm)
    assert kwargs["custom_llm_provider"] == "openai"
    assert kwargs["base_url"] == "https://opencode.ai/zen/v1"
    # The prefix drives litellm's bridge; it is stripped before the wire call.
    assert kwargs["model"] == "responses/muse-spark-1.3-contributor-free"


def test_api_mode_is_never_injected_into_environment() -> None:
    # The mode is UI-only form state: it must be readable for routing but must
    # never reach os.environ via temporary_env_and_lock at call time.
    llm = _make_opencode_llm("responses")
    assert OPENCODE_API_MODE_CONFIG_KEY in UI_ONLY_CONFIG_KEYS
    assert OPENCODE_API_MODE_CONFIG_KEY not in llm._env_only_custom_config
