from typing import Any
from uuid import UUID

from sqlalchemy import select, update
from sqlalchemy.orm import Session

from lumen.db.models import User, VoiceProvider
from lumen.error_handling.error_codes import LumenErrorCode
from lumen.error_handling.exceptions import LumenError

MIN_VOICE_PLAYBACK_SPEED = 0.5
MAX_VOICE_PLAYBACK_SPEED = 2.0


def fetch_voice_providers(db_session: Session) -> list[VoiceProvider]:
    """Fetch all voice providers."""
    return list(
        db_session.scalars(select(VoiceProvider).order_by(VoiceProvider.name)).all()
    )


def fetch_voice_provider_by_id(
    db_session: Session, provider_id: int
) -> VoiceProvider | None:
    """Fetch a voice provider by ID."""
    return db_session.scalar(
        select(VoiceProvider).where(VoiceProvider.id == provider_id)
    )


def fetch_default_stt_provider(db_session: Session) -> VoiceProvider | None:
    """Fetch the default STT provider."""
    return db_session.scalar(
        select(VoiceProvider).where(VoiceProvider.is_default_stt.is_(True))
    )


def fetch_default_tts_provider(db_session: Session) -> VoiceProvider | None:
    """Fetch the default TTS provider."""
    return db_session.scalar(
        select(VoiceProvider).where(VoiceProvider.is_default_tts.is_(True))
    )


def fetch_voice_provider_by_type(
    db_session: Session, provider_type: str
) -> VoiceProvider | None:
    """Fetch a voice provider by type."""
    return db_session.scalar(
        select(VoiceProvider).where(VoiceProvider.provider_type == provider_type)
    )


def upsert_voice_provider(
    *,
    db_session: Session,
    provider_id: int | None,
    name: str,
    provider_type: str,
    api_key: str | None,
    api_key_changed: bool,
    api_secret: str | None = None,
    api_secret_changed: bool = False,
    api_base: str | None = None,
    custom_config: dict[str, Any] | None = None,
    stt_model: str | None = None,
    tts_model: str | None = None,
    default_voice: str | None = None,
    activate_stt: bool = False,
    activate_tts: bool = False,
) -> VoiceProvider:
    """Create or update a voice provider."""
    provider: VoiceProvider | None = None

    if provider_id is not None:
        provider = fetch_voice_provider_by_id(db_session, provider_id)
        if provider is None:
            raise LumenError(
                LumenErrorCode.NOT_FOUND,
                f"No voice provider with id {provider_id} exists.",
            )
    else:
        provider = VoiceProvider()
        db_session.add(provider)

    # Apply updates
    provider.name = name
    provider.provider_type = provider_type
    provider.api_base = api_base
    # None means "leave unchanged" (pass {} to clear) so partial writers can't
    # wipe keys like speech_region.
    if custom_config is not None:
        provider.custom_config = custom_config
    # Same "None leaves it alone" rule: the STT and TTS setup modals each own
    # only their own fields, so neither can wipe the other's model or voice.
    if stt_model is not None:
        provider.stt_model = stt_model
    if tts_model is not None:
        provider.tts_model = tts_model
    if default_voice is not None:
        provider.default_voice = default_voice

    # Only update API key if explicitly changed or if provider has no key
    if api_key_changed or provider.api_key is None:
        provider.api_key = api_key  # ty: ignore[invalid-assignment]

    if api_secret_changed or provider.api_secret is None:
        provider.api_secret = api_secret  # ty: ignore[invalid-assignment]

    db_session.flush()

    if activate_stt:
        set_default_stt_provider(db_session=db_session, provider_id=provider.id)
    if activate_tts:
        set_default_tts_provider(db_session=db_session, provider_id=provider.id)

    db_session.refresh(provider)
    return provider


def clear_voice_provider_mode(
    *, db_session: Session, provider_id: int, mode: str
) -> VoiceProvider | None:
    """Drop one mode's configuration from a provider.

    A self-hosted row backs both STT and TTS, so disconnecting one mode must
    not take the other down with it. The row is deleted only once neither
    mode has a model left.

    Returns the surviving provider, or None when the row was deleted.
    """
    provider = fetch_voice_provider_by_id(db_session, provider_id)
    if provider is None:
        raise LumenError(
            LumenErrorCode.NOT_FOUND,
            f"No voice provider with id {provider_id} exists.",
        )

    if mode == "stt":
        provider.stt_model = None
        provider.is_default_stt = False
    else:
        provider.tts_model = None
        provider.default_voice = None
        provider.is_default_tts = False

    if provider.stt_model is None and provider.tts_model is None:
        db_session.delete(provider)
        db_session.flush()
        return None

    db_session.flush()
    db_session.refresh(provider)
    return provider


def delete_voice_provider(db_session: Session, provider_id: int) -> None:
    """Delete a voice provider by ID."""
    provider = fetch_voice_provider_by_id(db_session, provider_id)
    if provider:
        db_session.delete(provider)
        db_session.flush()


def set_default_stt_provider(*, db_session: Session, provider_id: int) -> VoiceProvider:
    """Set a voice provider as the default STT provider."""
    provider = fetch_voice_provider_by_id(db_session, provider_id)
    if provider is None:
        raise LumenError(
            LumenErrorCode.NOT_FOUND,
            f"No voice provider with id {provider_id} exists.",
        )

    # Deactivate all other STT providers
    db_session.execute(
        update(VoiceProvider)
        .where(
            VoiceProvider.is_default_stt.is_(True),
            VoiceProvider.id != provider_id,
        )
        .values(is_default_stt=False)
    )

    # Activate this provider
    provider.is_default_stt = True

    db_session.flush()
    db_session.refresh(provider)
    return provider


def set_default_tts_provider(
    *, db_session: Session, provider_id: int, tts_model: str | None = None
) -> VoiceProvider:
    """Set a voice provider as the default TTS provider."""
    provider = fetch_voice_provider_by_id(db_session, provider_id)
    if provider is None:
        raise LumenError(
            LumenErrorCode.NOT_FOUND,
            f"No voice provider with id {provider_id} exists.",
        )

    # Deactivate all other TTS providers
    db_session.execute(
        update(VoiceProvider)
        .where(
            VoiceProvider.is_default_tts.is_(True),
            VoiceProvider.id != provider_id,
        )
        .values(is_default_tts=False)
    )

    # Activate this provider
    provider.is_default_tts = True

    # Update the TTS model if specified
    if tts_model is not None:
        provider.tts_model = tts_model

    db_session.flush()
    db_session.refresh(provider)
    return provider


def deactivate_stt_provider(*, db_session: Session, provider_id: int) -> VoiceProvider:
    """Remove the default STT status from a voice provider."""
    provider = fetch_voice_provider_by_id(db_session, provider_id)
    if provider is None:
        raise LumenError(
            LumenErrorCode.NOT_FOUND,
            f"No voice provider with id {provider_id} exists.",
        )

    provider.is_default_stt = False

    db_session.flush()
    db_session.refresh(provider)
    return provider


def deactivate_tts_provider(*, db_session: Session, provider_id: int) -> VoiceProvider:
    """Remove the default TTS status from a voice provider."""
    provider = fetch_voice_provider_by_id(db_session, provider_id)
    if provider is None:
        raise LumenError(
            LumenErrorCode.NOT_FOUND,
            f"No voice provider with id {provider_id} exists.",
        )

    provider.is_default_tts = False

    db_session.flush()
    db_session.refresh(provider)
    return provider


# User voice preferences


def update_user_voice_settings(
    db_session: Session,
    user_id: UUID,
    auto_send: bool | None = None,
    auto_playback: bool | None = None,
    playback_speed: float | None = None,
) -> None:
    """Update user's voice settings.

    For all fields, None means "don't update this field".
    """
    values: dict[str, bool | float] = {}

    if auto_send is not None:
        values["voice_auto_send"] = auto_send
    if auto_playback is not None:
        values["voice_auto_playback"] = auto_playback
    if playback_speed is not None:
        values["voice_playback_speed"] = max(
            MIN_VOICE_PLAYBACK_SPEED, min(MAX_VOICE_PLAYBACK_SPEED, playback_speed)
        )

    if values:
        db_session.execute(
            update(User)
            .where(User.id == user_id)  # ty: ignore[invalid-argument-type]
            .values(**values)
        )
        db_session.flush()
