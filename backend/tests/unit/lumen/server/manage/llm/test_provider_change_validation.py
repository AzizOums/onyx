"""Tests for _validate_llm_provider_change's custom_config comparison.

Surface-mode keys pick a path on the same api_base, so changing them alone
must not force key re-entry; everything else still must.
"""

from unittest.mock import patch

import pytest

from lumen.error_handling.error_codes import LumenErrorCode
from lumen.error_handling.exceptions import LumenError
from lumen.llm.well_known_providers.constants import BIFROST_API_MODE_CONFIG_KEY
from lumen.server.manage.llm.api import _validate_llm_provider_change
from lumen.server.manage.llm.models import (
    LLMProviderUpsertRequest,
    normalize_extra_headers,
)

_BASE = "https://bifrost.example.com/v1"


def _validate(
    existing_custom_config: dict[str, str] | None,
    new_custom_config: dict[str, str] | None,
    new_api_base: str = _BASE,
) -> None:
    _validate_llm_provider_change(
        existing_api_base=_BASE,
        existing_custom_config=existing_custom_config,
        new_api_base=new_api_base,
        new_custom_config=new_custom_config,
        api_key_changed=False,
    )


@patch("lumen.server.manage.llm.api.MULTI_TENANT", True)
def test_surface_mode_only_change_is_allowed() -> None:
    _validate(None, {BIFROST_API_MODE_CONFIG_KEY: "responses"})
    _validate(
        {BIFROST_API_MODE_CONFIG_KEY: "chat_completions"},
        {BIFROST_API_MODE_CONFIG_KEY: "responses"},
    )


@patch("lumen.server.manage.llm.api.MULTI_TENANT", True)
def test_mode_change_alongside_unchanged_entries_is_allowed() -> None:
    _validate(
        {BIFROST_API_MODE_CONFIG_KEY: "chat_completions", "extra": "kept"},
        {BIFROST_API_MODE_CONFIG_KEY: "responses", "extra": "kept"},
    )


@patch("lumen.server.manage.llm.api.MULTI_TENANT", True)
def test_other_custom_config_change_still_rejected() -> None:
    with pytest.raises(LumenError):
        _validate(
            {BIFROST_API_MODE_CONFIG_KEY: "responses"},
            {BIFROST_API_MODE_CONFIG_KEY: "responses", "some_credential": "x"},
        )


@patch("lumen.server.manage.llm.api.MULTI_TENANT", True)
def test_mode_only_submission_dropping_stored_entries_is_rejected() -> None:
    # The submitted dict is persisted wholesale; dropped entries must reject.
    with pytest.raises(LumenError):
        _validate(
            {BIFROST_API_MODE_CONFIG_KEY: "chat_completions", "extra": "stored"},
            {BIFROST_API_MODE_CONFIG_KEY: "responses"},
        )


@patch("lumen.server.manage.llm.api.MULTI_TENANT", True)
def test_api_base_change_still_rejected() -> None:
    with pytest.raises(LumenError):
        _validate(
            {BIFROST_API_MODE_CONFIG_KEY: "responses"},
            {BIFROST_API_MODE_CONFIG_KEY: "responses"},
            new_api_base="https://attacker.example.com/v1",
        )


def test_single_tenant_skips_validation() -> None:
    # MULTI_TENANT is False in unit tests: everything passes.
    _validate(None, {"anything": "goes"})


def test_normalize_extra_headers_passthrough_none() -> None:
    assert normalize_extra_headers(None) is None


def test_normalize_extra_headers_empty_dict_becomes_none() -> None:
    assert normalize_extra_headers({}) is None


def test_normalize_extra_headers_strips_whitespace() -> None:
    assert normalize_extra_headers({"  User-Agent  ": "  opencode/1.18.18  "}) == {
        "User-Agent": "opencode/1.18.18"
    }


def test_normalize_extra_headers_rejects_authorization() -> None:
    with pytest.raises(LumenError) as exc_info:
        normalize_extra_headers({"Authorization": "Bearer secret"})
    assert exc_info.value.error_code == LumenErrorCode.BAD_REQUEST

    with pytest.raises(LumenError):
        normalize_extra_headers({"authorization": "Bearer secret"})


def test_normalize_extra_headers_rejects_bad_names() -> None:
    for bad_key in ["", "has space", "colon:name", "semi;colon", "é"]:
        with pytest.raises(LumenError):
            normalize_extra_headers({bad_key: "value"})


def test_normalize_extra_headers_rejects_empty_value() -> None:
    with pytest.raises(LumenError):
        normalize_extra_headers({"X-Custom": "   "})


def test_normalize_extra_headers_rejects_duplicates_after_strip() -> None:
    with pytest.raises(LumenError):
        normalize_extra_headers({"X-A": "1", "  X-A  ": "2"})


def test_normalize_extra_headers_enforces_count_limit() -> None:
    with pytest.raises(LumenError):
        normalize_extra_headers({f"X-H-{i}": "v" for i in range(33)})


def test_upsert_request_normalizes_extra_headers() -> None:
    request = LLMProviderUpsertRequest(
        provider="openai_compatible",
        api_key_changed=False,
        custom_config_changed=False,
        extra_headers={"User-Agent": "opencode/1.18.18"},
    )
    assert request.extra_headers == {"User-Agent": "opencode/1.18.18"}


def test_upsert_request_rejects_authorization_header() -> None:
    with pytest.raises(LumenError):
        LLMProviderUpsertRequest(
            provider="openai_compatible",
            api_key_changed=False,
            custom_config_changed=False,
            extra_headers={"Authorization": "Bearer secret"},
        )
