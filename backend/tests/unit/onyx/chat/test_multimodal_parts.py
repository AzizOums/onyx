"""Unit tests for multimodal message translation (audio/video parts)."""

from unittest.mock import patch

import json

from onyx.chat.llm_step import translate_history_to_llm_format
from onyx.llm.interfaces import LLMConfig
from onyx.chat.models import ChatLoadedFile, ChatMessageSimple
from onyx.configs.constants import MessageType
from onyx.file_store.models import ChatFileType
from onyx.llm.models import (
    InputAudioContentPart,
    TextContentPart,
    VideoContentPart,
)


def _make_config() -> LLMConfig:
    return LLMConfig(
        model_name="mimo-v2.5-free",
        model_provider="openai_compatible",
        temperature=0.0,
        max_input_tokens=200000,
    )


def _audio_file() -> ChatLoadedFile:
    return ChatLoadedFile(
        file_id="audio-1",
        content=b"RIFF....WAVEfake-bytes",
        file_type=ChatFileType.AUDIO,
        filename="clip.wav",
        content_text=None,
        token_count=0,
    )


def _video_file() -> ChatLoadedFile:
    return ChatLoadedFile(
        file_id="video-1",
        content=b"\x00\x00\x00\x18ftypmp42fake-bytes",
        file_type=ChatFileType.VIDEO,
        filename="clip.mp4",
        content_text=None,
        token_count=0,
    )


def test_audio_part_built_for_capable_model() -> None:
    msg = ChatMessageSimple(
        message="Listen",
        token_count=1,
        message_type=MessageType.USER,
        audio_files=[_audio_file()],
    )
    with (
        patch("onyx.chat.llm_step.model_supports_image_input", return_value=False),
        patch("onyx.chat.llm_step.model_supports_audio_input", return_value=True),
        patch("onyx.chat.llm_step.model_supports_video_input", return_value=False),
    ):
        from typing import cast

        result = translate_history_to_llm_format([msg], _make_config())
    messages = result if isinstance(result, list) else [result]
    parts = cast(list, messages[0].content)
    assert isinstance(parts, list)
    audio_parts = [p for p in parts if isinstance(p, InputAudioContentPart)]
    assert len(audio_parts) == 1
    assert audio_parts[0].input_audio.format == "wav"


def test_video_part_built_for_capable_model() -> None:
    msg = ChatMessageSimple(
        message="Watch",
        token_count=1,
        message_type=MessageType.USER,
        video_files=[_video_file()],
    )
    with (
        patch("onyx.chat.llm_step.model_supports_image_input", return_value=False),
        patch("onyx.chat.llm_step.model_supports_audio_input", return_value=False),
        patch("onyx.chat.llm_step.model_supports_video_input", return_value=True),
    ):
        from typing import cast

        result = translate_history_to_llm_format([msg], _make_config())
    messages = result if isinstance(result, list) else [result]
    parts = cast(list, messages[0].content)
    assert isinstance(parts, list)
    video_parts = [p for p in parts if isinstance(p, VideoContentPart)]
    assert len(video_parts) == 1
    assert video_parts[0].image_url.url.startswith("data:video/mp4;base64,")


def test_unsupported_modalities_degrade_to_text_markers() -> None:
    msg = ChatMessageSimple(
        message="Listen and watch",
        token_count=1,
        message_type=MessageType.USER,
        audio_files=[_audio_file()],
        video_files=[_video_file()],
    )
    with (
        patch("onyx.chat.llm_step.model_supports_image_input", return_value=False),
        patch("onyx.chat.llm_step.model_supports_audio_input", return_value=False),
        patch("onyx.chat.llm_step.model_supports_video_input", return_value=False),
    ):
        from typing import cast

        result = translate_history_to_llm_format([msg], _make_config())
    messages = result if isinstance(result, list) else [result]
    dumped = [m.model_dump() for m in messages]
    audio_video_parts = [
        p
        for m in dumped
        for p in (m["content"] if isinstance(m["content"], list) else [])
        if isinstance(p, dict) and p.get("type") in ("input_audio",)
        or isinstance(p, dict)
        and p.get("type") == "image_url"
        and str(p.get("image_url", {}).get("url", "")).startswith("data:video/")
    ]
    assert audio_video_parts == []
    flat = json.dumps(dumped)
    print("\nDUMPED:", flat[:600])
    assert "audio-1" in flat and "video-1" in flat
