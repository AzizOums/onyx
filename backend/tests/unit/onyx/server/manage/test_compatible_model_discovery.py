"""Unit tests for fetch_openai_compatible_model_ids and the
image-generation / voice available-models endpoints built on it."""

from unittest.mock import MagicMock, patch

import httpx
import pytest

from onyx.error_handling.error_codes import OnyxErrorCode
from onyx.error_handling.exceptions import OnyxError


def _models_response(model_ids: list[str]) -> MagicMock:
    response = MagicMock()
    response.json.return_value = {"data": [{"id": mid} for mid in model_ids]}
    response.raise_for_status.return_value = None
    return response


class TestFetchOpenAICompatibleModelIds:
    def test_returns_sorted_ids_without_embeddings(self) -> None:
        from onyx.server.manage.llm.utils import fetch_openai_compatible_model_ids

        with (
            patch(
                "onyx.server.manage.llm.utils.httpx.get",
                return_value=_models_response(
                    ["whisper-1", "tts-1", "flux-schnell", "text-embedding-3-small"]
                ),
            ),
            patch(
                "onyx.server.manage.llm.utils.is_embedding_model",
                side_effect=lambda name: "embedding" in name,
            ),
        ):
            assert fetch_openai_compatible_model_ids(
                "http://localhost:8080/v1", source_name="Image generation"
            ) == ["flux-schnell", "tts-1", "whisper-1"]

    def test_appends_v1_models_for_bare_base(self) -> None:
        from onyx.server.manage.llm.utils import fetch_openai_compatible_model_ids

        with (
            patch(
                "onyx.server.manage.llm.utils.httpx.get",
                return_value=_models_response(["m1"]),
            ) as mock_get,
            patch(
                "onyx.server.manage.llm.utils.is_embedding_model",
                return_value=False,
            ),
        ):
            fetch_openai_compatible_model_ids("http://localhost:8080")
            assert mock_get.call_args.args[0] == "http://localhost:8080/v1/models"

    def test_unauthorized_maps_to_400(self) -> None:
        from onyx.server.manage.llm.utils import fetch_openai_compatible_model_ids

        error_response = httpx.Response(
            status_code=401, request=httpx.Request("GET", "http://x/v1/models")
        )
        with patch(
            "onyx.server.manage.llm.utils.httpx.get",
            side_effect=httpx.HTTPStatusError(
                "unauthorized",
                request=error_response.request,
                response=error_response,
            ),
        ):
            with pytest.raises(OnyxError) as exc_info:
                fetch_openai_compatible_model_ids("http://x")
        assert exc_info.value.error_code == OnyxErrorCode.VALIDATION_ERROR
        assert exc_info.value.status_code == 400

    def test_unreachable_maps_to_502(self) -> None:
        from onyx.server.manage.llm.utils import fetch_openai_compatible_model_ids

        with patch(
            "onyx.server.manage.llm.utils.httpx.get",
            side_effect=httpx.ConnectError("refused", request=MagicMock()),
        ):
            with pytest.raises(OnyxError) as exc_info:
                fetch_openai_compatible_model_ids("http://x")
        assert exc_info.value.error_code == OnyxErrorCode.BAD_GATEWAY


class TestImageGenAvailableModelsEndpoint:
    def test_returns_names(self) -> None:
        from onyx.server.manage.image_generation.api import list_available_models
        from onyx.server.manage.image_generation.models import (
            AvailableImageModelsRequest,
        )

        with patch(
            "onyx.server.manage.image_generation.api.fetch_openai_compatible_model_ids",
            return_value=["flux-schnell"],
        ) as mock_fetch:
            result = list_available_models(
                AvailableImageModelsRequest(api_base="http://localhost:8080/v1"),
                MagicMock(),
            )
        mock_fetch.assert_called_once_with(
            api_base="http://localhost:8080/v1",
            api_key=None,
            source_name="Image generation",
        )
        assert [m.name for m in result] == ["flux-schnell"]


class TestVoiceAvailableModelsEndpoint:
    def test_returns_ids(self) -> None:
        from onyx.server.manage.voice.api import list_available_models
        from onyx.server.manage.voice.models import AvailableVoiceModelsRequest

        with patch(
            "onyx.server.manage.voice.api.fetch_openai_compatible_model_ids",
            return_value=["whisper-1"],
        ) as mock_fetch:
            result = list_available_models(
                AvailableVoiceModelsRequest(api_base="http://localhost:8080/v1"),
                MagicMock(),
            )
        mock_fetch.assert_called_once_with(
            api_base="http://localhost:8080/v1",
            api_key=None,
            source_name="Voice",
        )
        assert [m.id for m in result] == ["whisper-1"]
