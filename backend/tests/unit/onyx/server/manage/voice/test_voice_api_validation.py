import pytest

from onyx.error_handling.exceptions import OnyxError
from onyx.server.manage.voice.api import _validate_voice_api_base


def test_validate_voice_api_base_blocks_private_for_other_types() -> None:
    with pytest.raises(OnyxError, match="Invalid target URI"):
        _validate_voice_api_base("elevenlabs", "http://127.0.0.1:11434")


def test_validate_voice_api_base_allows_private_for_openai() -> None:
    # Self-hosted OpenAI-compatible servers (LocalAI, Kokoro, whisper)
    # live on private networks; the admin-typed base URL is trusted like
    # the Azure target URI.
    validated = _validate_voice_api_base("openai", "http://127.0.0.1:11434")
    assert validated == "http://127.0.0.1:11434"


def test_validate_voice_api_base_allows_private_for_azure() -> None:
    validated = _validate_voice_api_base("azure", "http://127.0.0.1:5000")
    assert validated == "http://127.0.0.1:5000"


def test_validate_voice_api_base_allows_private_for_openai_compatible() -> None:
    validated = _validate_voice_api_base("openai_compatible", "http://127.0.0.1:11434")
    assert validated == "http://127.0.0.1:11434"


def test_openai_compatible_factory_returns_openai_provider() -> None:
    from unittest.mock import MagicMock

    from onyx.voice.factory import get_voice_provider
    from onyx.voice.providers.openai import OpenAIVoiceProvider

    provider = MagicMock()
    provider.provider_type = "openai_compatible"
    provider.api_key = None
    provider.api_base = "http://127.0.0.1:8080/v1"
    provider.custom_config = None
    provider.stt_model = "whisper-1"
    provider.tts_model = "kokoro"
    provider.default_voice = "af_bella"

    voice_provider = get_voice_provider(provider)
    assert isinstance(voice_provider, OpenAIVoiceProvider)
    assert voice_provider.stt_model == "whisper-1"
    assert voice_provider.tts_model == "kokoro"


def test_validate_voice_api_base_blocks_metadata_for_azure() -> None:
    with pytest.raises(OnyxError, match="Invalid target URI"):
        _validate_voice_api_base("azure", "http://metadata.google.internal/")


def test_validate_voice_api_base_returns_none_for_none() -> None:
    assert _validate_voice_api_base("openai", None) is None
