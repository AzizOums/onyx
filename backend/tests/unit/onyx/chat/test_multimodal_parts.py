"""Unit tests for multimodal message translation (audio/video parts)."""

import json
from unittest.mock import patch

from onyx.chat.llm_step import translate_history_to_llm_format
from onyx.chat.models import ChatLoadedFile, ChatMessageSimple
from onyx.configs.constants import MessageType
from onyx.file_store.models import ChatFileType
from onyx.llm.interfaces import LLMConfig
from onyx.llm.models import (
    InputAudioContentPart,
    VideoContentPart,
    VideoUrlContentPart,
)


def _make_config(provider: str = "openai_compatible") -> LLMConfig:
    return LLMConfig(
        model_name="mimo-v2.5-free",
        model_provider=provider,
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


def test_opencode_video_part_uses_native_video_url() -> None:
    """Xiaomi MiMo rejects image_url parts carrying a video MIME, so the
    opencode provider must emit the native video_url part."""
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

        result = translate_history_to_llm_format([msg], _make_config("opencode"))
    messages = result if isinstance(result, list) else [result]
    parts = cast(list, messages[0].content)
    assert isinstance(parts, list)
    assert not [p for p in parts if isinstance(p, VideoContentPart)]
    video_parts = [p for p in parts if isinstance(p, VideoUrlContentPart)]
    assert len(video_parts) == 1
    assert video_parts[0].video_url.url.startswith("data:video/mp4;base64,")
    dumped = messages[0].model_dump()["content"]
    assert dumped[2] == {
        "type": "video_url",
        "video_url": {"url": video_parts[0].video_url.url},
    }


def test_opencode_unsupported_video_format_degrades_to_marker() -> None:
    video = _video_file()
    video.filename = "clip.webm"
    msg = ChatMessageSimple(
        message="Watch",
        token_count=1,
        message_type=MessageType.USER,
        video_files=[video],
    )
    with (
        patch("onyx.chat.llm_step.model_supports_image_input", return_value=False),
        patch("onyx.chat.llm_step.model_supports_audio_input", return_value=False),
        patch("onyx.chat.llm_step.model_supports_video_input", return_value=True),
    ):
        from typing import cast

        result = translate_history_to_llm_format([msg], _make_config("opencode"))
    messages = result if isinstance(result, list) else [result]
    parts = cast(list, messages[0].content)
    assert isinstance(parts, list)
    assert not [p for p in parts if isinstance(p, VideoUrlContentPart)]
    flat = json.dumps([m.model_dump() for m in messages])
    assert "video-1" in flat and "not supported" in flat


def test_opencode_oversized_video_degrades_to_marker() -> None:
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
        patch("onyx.chat.llm_step.OPENCODE_MAX_VIDEO_BASE64_CHARS", 4),
    ):
        from typing import cast

        result = translate_history_to_llm_format([msg], _make_config("opencode"))
    messages = result if isinstance(result, list) else [result]
    parts = cast(list, messages[0].content)
    assert isinstance(parts, list)
    assert not [p for p in parts if isinstance(p, VideoUrlContentPart)]
    flat = json.dumps([m.model_dump() for m in messages])
    assert "video-1" in flat and "too large" in flat
