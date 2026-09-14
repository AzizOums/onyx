"""OpenAI voice provider for STT and TTS.

- STT chunked: `/v1/audio/transcriptions` with the provider's `stt_model`.
- STT streaming: GA Realtime WS, always `gpt-realtime-whisper`. No server-side
  VAD on this model, so `close()` sends an explicit `input_audio_buffer.commit`.
- TTS: HTTP streaming, tts-1 / tts-1-hd.
"""

import asyncio
import base64
import io
import json
from collections.abc import AsyncIterator
from enum import StrEnum
from typing import TYPE_CHECKING, Any

import aiohttp

from lumen.tracing.flows import LLMFlow
from lumen.tracing.llm_utils import traced_llm_call
from lumen.voice.interface import (
    StreamingSynthesizerProtocol,
    StreamingTranscriberProtocol,
    TranscriptResult,
    VoiceProviderInterface,
)

if TYPE_CHECKING:
    from openai import AsyncOpenAI

# Default OpenAI API base URL
DEFAULT_OPENAI_API_BASE = "https://api.openai.com"


# GA Realtime WS needs two model slots: the realtime *session* model in the
# URL `?model=` (omit → `missing_model`), and the *transcription* model in
# `audio.input.transcription.model`. Other STT models stay HTTP-only.
OPENAI_REALTIME_SESSION_MODEL = "gpt-realtime-1.5"
OPENAI_REALTIME_STT_MODEL = "gpt-realtime-whisper"


class OpenAIRealtimeMessageType(StrEnum):
    """Message types from OpenAI Realtime transcription API (GA shape)."""

    ERROR = "error"
    SPEECH_STARTED = "input_audio_buffer.speech_started"
    SPEECH_STOPPED = "input_audio_buffer.speech_stopped"
    BUFFER_COMMITTED = "input_audio_buffer.committed"
    TRANSCRIPTION_DELTA = "conversation.item.input_audio_transcription.delta"
    TRANSCRIPTION_COMPLETED = "conversation.item.input_audio_transcription.completed"
    SESSION_CREATED = "session.created"
    SESSION_UPDATED = "session.updated"
    ITEM_CREATED = "conversation.item.created"


def normalize_openai_api_root(api_base: str | None) -> str:
    """Server root for an OpenAI-compatible base URL, with no trailing `/v1`.

    Admins paste either form (`http://host:8002` or `http://host:8002/v1`);
    callers that build `/v1/...` paths themselves need the bare root.
    """
    root = (api_base or DEFAULT_OPENAI_API_BASE).strip().rstrip("/")
    if root.endswith("/v1"):
        root = root[: -len("/v1")]
    return root


def _http_to_ws_url(http_url: str) -> str:
    """Convert http(s) URL to ws(s) URL for WebSocket connections."""
    if http_url.startswith("https://"):
        return "wss://" + http_url[8:]
    elif http_url.startswith("http://"):
        return "ws://" + http_url[7:]
    return http_url


class OpenAIStreamingTranscriber(StreamingTranscriberProtocol):
    """Streaming transcription using OpenAI Realtime API."""

    def __init__(
        self,
        api_key: str,
        model: str = OPENAI_REALTIME_STT_MODEL,
        api_base: str | None = None,
    ):
        # Import logger first
        from lumen.utils.logger import setup_logger

        self._logger = setup_logger()

        self._logger.info(
            "OpenAIStreamingTranscriber: initializing with model %s", model
        )
        self.api_key = api_key
        self.model = model
        self.api_base = normalize_openai_api_root(api_base)
        self._ws: aiohttp.ClientWebSocketResponse | None = None
        self._session: aiohttp.ClientSession | None = None
        self._transcript_queue: asyncio.Queue[TranscriptResult | None] = asyncio.Queue()
        self._current_turn_transcript = ""  # Transcript for current VAD turn
        self._accumulated_transcript = ""  # Accumulated across all turns
        self._receive_task: asyncio.Task | None = None
        self._closed = False
        # OpenAI keeps the WS open on protocol errors, so we cache the last
        # error event for callers to fail fast on a poisoned session.
        self._last_error: dict[str, Any] | None = None

    async def connect(self) -> None:
        """Establish WebSocket connection to OpenAI Realtime API (GA shape)."""
        self._session = aiohttp.ClientSession()

        # `?model=` must be the realtime session model — a transcription
        # model here yields `invalid_model`. The Beta `?intent=transcription`
        # form now returns `beta_api_shape_disabled`.
        ws_base = _http_to_ws_url(self.api_base)
        url = f"{ws_base}/v1/realtime?model={OPENAI_REALTIME_SESSION_MODEL}"
        headers = {"Authorization": f"Bearer {self.api_key}"}

        try:
            self._ws = await self._session.ws_connect(url, headers=headers)
            self._logger.info("Connected to OpenAI Realtime API")
        except Exception as e:
            self._logger.error("Failed to connect to OpenAI Realtime API: %s", e)
            raise

        # `session.type` must be `"realtime"`. GA has no transcription-only
        # session type on the WS — transcription is a side-effect configured
        # via `audio.input.transcription.model`. `"transcription"` here →
        # "Passing a transcription session update event to a realtime
        # session is not allowed". `turn_detection: None` because
        # gpt-realtime-whisper has no server-side VAD; `close()` drives
        # turn boundaries via `input_audio_buffer.commit`.
        config_message = {
            "type": "session.update",
            "session": {
                "type": "realtime",
                "audio": {
                    "input": {
                        "format": {"type": "audio/pcm", "rate": 24000},
                        "transcription": {"model": self.model},
                        "turn_detection": None,
                    },
                },
            },
        }
        await self._ws.send_str(json.dumps(config_message))
        self._logger.info("Sent config for model: %s", self.model)

        # Start receiving transcripts
        self._receive_task = asyncio.create_task(self._receive_loop())

    async def _receive_loop(self) -> None:
        """Background task to receive transcripts."""
        if not self._ws:
            return

        try:
            async for msg in self._ws:
                if msg.type == aiohttp.WSMsgType.TEXT:
                    data = json.loads(msg.data)
                    msg_type = data.get("type", "")
                    self._logger.debug("Received message type: %s", msg_type)

                    # WS stays open on protocol errors — cache for callers.
                    if msg_type == OpenAIRealtimeMessageType.ERROR:
                        error = data.get("error", {})
                        self._last_error = error
                        self._logger.error("OpenAI error: %s", error)
                        continue

                    # speech_started/stopped are no-ops on gpt-realtime-whisper
                    # (no server VAD) but kept defensively. buffer_committed
                    # fires once per session from close()'s manual commit.
                    if msg_type == OpenAIRealtimeMessageType.SPEECH_STARTED:
                        self._logger.info("OpenAI: Speech started")
                        self._current_turn_transcript = ""
                        continue
                    elif msg_type == OpenAIRealtimeMessageType.SPEECH_STOPPED:
                        self._logger.info("OpenAI: Speech stopped")
                        continue
                    elif msg_type == OpenAIRealtimeMessageType.BUFFER_COMMITTED:
                        self._logger.info("OpenAI: Audio buffer committed")
                        continue

                    # Handle transcription events
                    if msg_type == OpenAIRealtimeMessageType.TRANSCRIPTION_DELTA:
                        delta = data.get("delta", "")
                        if delta:
                            self._logger.info("OpenAI: Transcription delta: %s", delta)
                            self._current_turn_transcript += delta
                            # Show accumulated + current turn transcript
                            full_transcript = self._accumulated_transcript
                            if full_transcript and self._current_turn_transcript:
                                full_transcript += " "
                            full_transcript += self._current_turn_transcript
                            await self._transcript_queue.put(
                                TranscriptResult(text=full_transcript, is_vad_end=False)
                            )
                    elif msg_type == OpenAIRealtimeMessageType.TRANSCRIPTION_COMPLETED:
                        transcript = data.get("transcript", "")
                        if transcript:
                            self._logger.info(
                                "OpenAI: Transcription completed (VAD turn end): %s...",
                                transcript[:50],
                            )
                            # This is the final transcript for this VAD turn
                            self._current_turn_transcript = transcript
                            # Accumulate this turn's transcript
                            if self._accumulated_transcript:
                                self._accumulated_transcript += " " + transcript
                            else:
                                self._accumulated_transcript = transcript
                            # Send with is_vad_end=True to trigger auto-send
                            await self._transcript_queue.put(
                                TranscriptResult(
                                    text=self._accumulated_transcript,
                                    is_vad_end=True,
                                )
                            )
                    elif msg_type not in (
                        OpenAIRealtimeMessageType.SESSION_CREATED,
                        OpenAIRealtimeMessageType.SESSION_UPDATED,
                        OpenAIRealtimeMessageType.ITEM_CREATED,
                    ):
                        # Log any other message types we might be missing
                        self._logger.info(
                            "OpenAI: Unhandled message type '%s': %s", msg_type, data
                        )

                elif msg.type == aiohttp.WSMsgType.ERROR:
                    self._logger.error("WebSocket error: %s", self._ws.exception())
                    break
                elif msg.type == aiohttp.WSMsgType.CLOSED:
                    self._logger.info("WebSocket closed by server")
                    break
        except Exception as e:
            self._logger.error("Error in receive loop: %s", e)
        finally:
            await self._transcript_queue.put(None)

    async def send_audio(self, chunk: bytes) -> None:
        """Send audio chunk to OpenAI."""
        if self._ws and not self._closed:
            # OpenAI expects base64-encoded PCM16 audio at 24kHz mono
            # PCM16 at 24kHz: 24000 samples/sec * 2 bytes/sample = 48000 bytes/sec
            # So chunk_bytes / 48000 = duration in seconds
            duration_ms = (len(chunk) / 48000) * 1000
            self._logger.debug(
                "Sending %s bytes (%sms) of audio to OpenAI. First 10 bytes: %s",
                len(chunk),
                format(duration_ms, ".1f"),
                chunk[:10].hex() if len(chunk) >= 10 else chunk.hex(),
            )
            message = {
                "type": "input_audio_buffer.append",
                "audio": base64.b64encode(chunk).decode("utf-8"),
            }
            await self._ws.send_str(json.dumps(message))

    def reset_transcript(self) -> None:
        """Reset accumulated transcript. Call after auto-send to start fresh."""
        self._logger.info("OpenAI: Resetting accumulated transcript")
        self._accumulated_transcript = ""
        self._current_turn_transcript = ""

    async def receive_transcript(self) -> TranscriptResult | None:
        """Receive next transcript."""
        try:
            return await asyncio.wait_for(self._transcript_queue.get(), timeout=0.1)
        except asyncio.TimeoutError:
            return TranscriptResult(text="", is_vad_end=False)

    async def close(self) -> str:
        """Close session and return final transcript."""
        self._closed = True
        if self._ws:
            # Sole trigger for the final TRANSCRIPTION_COMPLETED event
            # (gpt-realtime-whisper has no server VAD).
            try:
                await self._ws.send_str(
                    json.dumps({"type": "input_audio_buffer.commit"})
                )
            except Exception as e:
                self._logger.debug("Error sending commit (may be expected): %s", e)

            # Wait for *new* transcription to arrive (up to 5 seconds)
            self._logger.info("Waiting for transcription to complete...")
            transcript_before_commit = self._accumulated_transcript
            for _ in range(50):  # 50 * 100ms = 5 seconds max
                await asyncio.sleep(0.1)
                if self._accumulated_transcript != transcript_before_commit:
                    self._logger.info(
                        "Got final transcript: %s...", self._accumulated_transcript[:50]
                    )
                    break
            else:
                self._logger.warning("Timed out waiting for transcription")

            await self._ws.close()
        if self._receive_task:
            self._receive_task.cancel()
            try:
                await self._receive_task
            except asyncio.CancelledError:
                pass
        if self._session:
            await self._session.close()
        return self._accumulated_transcript


# OpenAI available voices for TTS
OPENAI_VOICES = [
    {"id": "alloy", "name": "Alloy"},
    {"id": "echo", "name": "Echo"},
    {"id": "fable", "name": "Fable"},
    {"id": "lumen", "name": "Lumen"},
    {"id": "nova", "name": "Nova"},
    {"id": "shimmer", "name": "Shimmer"},
]

# Models for the chunked HTTP path only — streaming is locked to
# OPENAI_REALTIME_STT_MODEL regardless of selection.
OPENAI_STT_MODELS = [
    {"id": "whisper-1", "name": "Whisper v1"},
    {"id": "gpt-4o-transcribe", "name": "GPT-4o Transcribe"},
    {"id": "gpt-4o-mini-transcribe", "name": "GPT-4o Mini Transcribe"},
]

# OpenAI available TTS models
OPENAI_TTS_MODELS = [
    {"id": "tts-1", "name": "TTS-1 (Standard)"},
    {"id": "tts-1-hd", "name": "TTS-1 HD (High Quality)"},
]


def _create_wav_header(
    data_length: int,
    sample_rate: int = 24000,
    channels: int = 1,
    bits_per_sample: int = 16,
) -> bytes:
    """Create a WAV file header for PCM audio data."""
    import struct

    byte_rate = sample_rate * channels * bits_per_sample // 8
    block_align = channels * bits_per_sample // 8

    # WAV header is 44 bytes
    header = struct.pack(
        "<4sI4s4sIHHIIHH4sI",
        b"RIFF",  # ChunkID
        36 + data_length,  # ChunkSize
        b"WAVE",  # Format
        b"fmt ",  # Subchunk1ID
        16,  # Subchunk1Size (PCM)
        1,  # AudioFormat (1 = PCM)
        channels,  # NumChannels
        sample_rate,  # SampleRate
        byte_rate,  # ByteRate
        block_align,  # BlockAlign
        bits_per_sample,  # BitsPerSample
        b"data",  # Subchunk2ID
        data_length,  # Subchunk2Size
    )
    return header


class OpenAIStreamingSynthesizer(StreamingSynthesizerProtocol):
    """Streaming TTS using OpenAI HTTP TTS API with streaming responses."""

    def __init__(
        self,
        api_key: str,
        voice: str = "alloy",
        model: str = "tts-1",
        speed: float = 1.0,
        api_base: str | None = None,
    ):
        from lumen.utils.logger import setup_logger

        self._logger = setup_logger()
        self.api_key = api_key
        self.voice = voice
        self.model = model
        self.speed = max(0.25, min(4.0, speed))
        self.api_base = normalize_openai_api_root(api_base)
        self._session: aiohttp.ClientSession | None = None
        self._audio_queue: asyncio.Queue[bytes | None] = asyncio.Queue()
        self._text_queue: asyncio.Queue[str | None] = asyncio.Queue()
        self._synthesis_task: asyncio.Task | None = None
        self._closed = False
        self._flushed = False

    async def connect(self) -> None:
        """Initialize HTTP session for TTS requests."""
        self._logger.info("OpenAIStreamingSynthesizer: connecting")
        self._session = aiohttp.ClientSession()
        # Start background task to process text queue
        self._synthesis_task = asyncio.create_task(self._process_text_queue())
        self._logger.info("OpenAIStreamingSynthesizer: connected")

    async def _process_text_queue(self) -> None:
        """Background task to process queued text for synthesis."""
        while not self._closed:
            try:
                text = await asyncio.wait_for(self._text_queue.get(), timeout=0.1)
                if text is None:
                    break
                await self._synthesize_text(text)
            except asyncio.TimeoutError:
                continue
            except asyncio.CancelledError:
                break
            except Exception as e:
                self._logger.error("Error processing text queue: %s", e)

    async def _synthesize_text(self, text: str) -> None:
        """Make HTTP TTS request and stream audio to queue."""
        if not self._session or self._closed:
            return

        url = f"{self.api_base}/v1/audio/speech"
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }
        payload = {
            "model": self.model,
            "voice": self.voice,
            "input": text,
            "speed": self.speed,
            "response_format": "mp3",
        }

        try:
            async with self._session.post(
                url, headers=headers, json=payload
            ) as response:
                if response.status != 200:
                    error_text = await response.text()
                    self._logger.error("OpenAI TTS error: %s", error_text)
                    return

                # Use 8192 byte chunks for smoother streaming
                # (larger chunks = more complete MP3 frames, better playback)
                async for chunk in response.content.iter_chunked(8192):
                    if self._closed:
                        break
                    if chunk:
                        await self._audio_queue.put(chunk)
        except Exception as e:
            self._logger.error("OpenAIStreamingSynthesizer synthesis error: %s", e)

    async def send_text(self, text: str) -> None:
        """Queue text to be synthesized via HTTP streaming."""
        if not text.strip() or self._closed:
            return
        await self._text_queue.put(text)

    async def receive_audio(self) -> bytes | None:
        """Receive next audio chunk (MP3 format)."""
        try:
            return await asyncio.wait_for(self._audio_queue.get(), timeout=0.1)
        except asyncio.TimeoutError:
            return b""  # No audio yet, but not done

    async def flush(self) -> None:
        """Signal end of text input - wait for synthesis to complete."""
        if self._flushed:
            return
        self._flushed = True

        # Signal end of text input
        await self._text_queue.put(None)

        # Wait for synthesis task to complete processing all text
        if self._synthesis_task and not self._synthesis_task.done():
            try:
                await asyncio.wait_for(self._synthesis_task, timeout=60.0)
            except asyncio.TimeoutError:
                self._logger.warning("OpenAIStreamingSynthesizer: flush timeout")
                self._synthesis_task.cancel()
                try:
                    await self._synthesis_task
                except asyncio.CancelledError:
                    pass
            except asyncio.CancelledError:
                pass

        # Signal end of audio stream
        await self._audio_queue.put(None)

    async def close(self) -> None:
        """Close the session."""
        if self._closed:
            return
        self._closed = True

        # Signal end of queues only if flush wasn't already called
        if not self._flushed:
            await self._text_queue.put(None)
            await self._audio_queue.put(None)

        if self._synthesis_task and not self._synthesis_task.done():
            self._synthesis_task.cancel()
            try:
                await self._synthesis_task
            except asyncio.CancelledError:
                pass

        if self._session:
            await self._session.close()


class OpenAIVoiceProvider(VoiceProviderInterface):
    """OpenAI voice provider using Whisper for STT and TTS API for speech synthesis."""

    def __init__(
        self,
        api_key: str | None,
        api_base: str | None = None,
        stt_model: str | None = None,
        tts_model: str | None = None,
        default_voice: str | None = None,
        self_hosted: bool = False,
    ):
        self.api_key = api_key
        self.api_base = api_base
        # A self-hosted server names its own models and voices, so the OpenAI
        # catalogue defaults must not leak into it.
        self.self_hosted = self_hosted
        self.stt_model = stt_model or (None if self_hosted else "whisper-1")
        self.tts_model = tts_model or (None if self_hosted else "tts-1")
        self.default_voice = default_voice or (None if self_hosted else "alloy")

        self._client: "AsyncOpenAI | None" = None

    def _get_client(self) -> "AsyncOpenAI":
        if self._client is None:
            from openai import AsyncOpenAI

            # The SDK appends `/audio/speech` to base_url, so the base must
            # carry the `/v1` suffix whichever form the admin pasted.
            self._client = AsyncOpenAI(
                api_key=self.api_key,
                base_url=(
                    f"{normalize_openai_api_root(self.api_base)}/v1"
                    if self.api_base
                    else None
                ),
            )
        return self._client

    async def transcribe(self, audio_data: bytes, audio_format: str) -> str:
        """Transcribe audio via `/v1/audio/transcriptions`."""
        client = self._get_client()

        # /v1/audio/transcriptions doesn't accept raw PCM — wrap as WAV
        # (24kHz mono matches the browser capture format).
        if audio_format == "pcm16":
            audio_data = _create_wav_header(len(audio_data)) + audio_data
            audio_format = "wav"

        # Create a file-like object from the audio bytes
        audio_file = io.BytesIO(audio_data)
        audio_file.name = f"audio.{audio_format}"

        stt_model = self.stt_model or "whisper-1"
        with traced_llm_call(
            flow=LLMFlow.STT,
            model=stt_model,
            provider="openai",
        ):
            response = await client.audio.transcriptions.create(
                model=stt_model,
                file=audio_file,
            )

        return response.text

    async def synthesize_stream(
        self, text: str, voice: str | None = None, speed: float = 1.0
    ) -> AsyncIterator[bytes]:
        """
        Convert text to audio using OpenAI TTS with streaming.

        Args:
            text: Text to convert to speech
            voice: Voice identifier (defaults to provider's default voice)
            speed: Playback speed multiplier (0.25 to 4.0)

        Yields:
            Audio data chunks (mp3 format)
        """
        client = self._get_client()

        # Clamp speed to valid range
        speed = max(0.25, min(4.0, speed))

        # Use with_streaming_response for proper async streaming
        # Using 8192 byte chunks for better streaming performance
        # (larger chunks = fewer round-trips, more complete MP3 frames)
        tts_model = self.tts_model or "tts-1"
        # A self-hosted server has its own speaker names, so there is no safe
        # default to fall back on — say so rather than send an OpenAI name it
        # will reject.
        selected_voice = voice or self.default_voice
        if not selected_voice:
            raise ValueError(
                "No voice configured for this provider. Set a voice that "
                f"the model '{tts_model}' offers."
            )
        with traced_llm_call(
            flow=LLMFlow.TTS,
            model=tts_model,
            provider="openai",
            input_messages=[{"role": "user", "content": text}],
        ):
            async with client.audio.speech.with_streaming_response.create(
                model=tts_model,
                voice=selected_voice,
                input=text,
                speed=speed,
                response_format="mp3",
            ) as response:
                async for chunk in response.iter_bytes(chunk_size=8192):
                    yield chunk

    async def validate_credentials(self) -> None:
        """Validate OpenAI API key by listing models."""
        from openai import AuthenticationError, PermissionDeniedError

        client = self._get_client()
        try:
            await client.models.list()
        except AuthenticationError:
            raise RuntimeError("Invalid OpenAI API key.")
        except PermissionDeniedError:
            raise RuntimeError("OpenAI API key does not have sufficient permissions.")

    def get_available_voices(self) -> list[dict[str, str]]:
        """Get available OpenAI TTS voices.

        A self-hosted server ships its own speakers, so it gets no static
        list — `list_voices` asks the server instead.
        """
        if self.self_hosted:
            return []
        return OPENAI_VOICES.copy()

    async def list_voices(self) -> list[dict[str, str]]:
        """Voices for the configured TTS model.

        Self-hosted servers expose `GET /v1/audio/voices?model=...` (the
        voice-pack convention used by Kokoro and friends). Models that ship
        no voice packs answer with an empty list, and models the server
        doesn't know answer with an error — both mean "let the admin type
        the speaker name", so neither is fatal.
        """
        if not self.self_hosted:
            return self.get_available_voices()

        root = normalize_openai_api_root(self.api_base)
        url = f"{root}/v1/audio/voices"
        params = {"model": self.tts_model} if self.tts_model else {}
        headers = {"Authorization": f"Bearer {self.api_key}"} if self.api_key else {}

        from lumen.utils.logger import setup_logger

        logger = setup_logger()
        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(
                    url,
                    params=params,
                    headers=headers,
                    timeout=aiohttp.ClientTimeout(total=15),
                ) as response:
                    if response.status != 200:
                        logger.info(
                            "Voice server returned %s for %s; no voice list.",
                            response.status,
                            url,
                        )
                        return []
                    payload = await response.json()
        except Exception as e:
            logger.info("Could not list voices at %s: %s", url, e)
            return []

        entries = payload.get("data") if isinstance(payload, dict) else payload
        if not isinstance(entries, list):
            return []

        voices: list[dict[str, str]] = []
        for entry in entries:
            if isinstance(entry, str):
                voices.append({"id": entry, "name": entry})
            elif isinstance(entry, dict):
                voice_id = entry.get("id") or entry.get("voice") or entry.get("name")
                if voice_id:
                    voices.append(
                        {
                            "id": str(voice_id),
                            "name": str(entry.get("name") or voice_id),
                        }
                    )
        return voices

    def get_available_stt_models(self) -> list[dict[str, str]]:
        """Get available OpenAI STT models."""
        return OPENAI_STT_MODELS.copy()

    def get_available_tts_models(self) -> list[dict[str, str]]:
        """Get available OpenAI TTS models."""
        return OPENAI_TTS_MODELS.copy()

    def supports_streaming_stt(self) -> bool:
        # Streaming WS is locked to OPENAI_REALTIME_STT_MODEL; stt_model
        # only governs the chunked HTTP path. Self-hosted servers implement
        # the REST audio routes but not the Realtime WS, so attempting it
        # only costs a rejected handshake before the chunked fallback.
        return not self.self_hosted

    def supports_streaming_tts(self) -> bool:
        """OpenAI supports real-time streaming TTS via Realtime API."""
        return True

    async def create_streaming_transcriber(  # ty: ignore[invalid-method-override]
        self, _audio_format: str = "webm"
    ) -> OpenAIStreamingTranscriber:
        # Streaming always uses OPENAI_REALTIME_STT_MODEL — other STT models
        # are not Realtime-streamable.
        if not self.api_key:
            raise ValueError("API key required for streaming transcription")
        transcriber = OpenAIStreamingTranscriber(
            api_key=self.api_key,
            model=OPENAI_REALTIME_STT_MODEL,
            api_base=self.api_base,
        )
        await transcriber.connect()
        return transcriber

    async def create_streaming_synthesizer(
        self, voice: str | None = None, speed: float = 1.0
    ) -> OpenAIStreamingSynthesizer:
        """Create a streaming TTS session using HTTP streaming API."""
        if not self.api_key:
            raise ValueError("API key required for streaming TTS")
        selected_voice = voice or self.default_voice
        if not selected_voice:
            if self.self_hosted:
                raise ValueError(
                    "No voice configured for this provider. Set a voice that "
                    "your server offers for the selected model."
                )
            selected_voice = "alloy"
        synthesizer = OpenAIStreamingSynthesizer(
            api_key=self.api_key,
            voice=selected_voice,
            model=self.tts_model or "tts-1",
            speed=speed,
            api_base=self.api_base,
        )
        await synthesizer.connect()
        return synthesizer
