from typing import cast

from fastapi import APIRouter, Depends
from sqlalchemy.exc import SQLAlchemyError
from sqlalchemy.orm import Session

from lumen import __version__ as lumen_version
from lumen.auth.permissions import require_permission
from lumen.auth.users import is_user_admin
from lumen.configs.app_configs import (
    DEFAULT_USER_FILE_MAX_UPLOAD_SIZE_MB,
    DISABLE_VECTOR_DB,
    MAX_ALLOWED_UPLOAD_SIZE_MB,
    POSTHOG_API_KEY,
    POSTHOG_HOST,
)
from lumen.configs.constants import KV_REINDEX_KEY, NotificationType
from lumen.db.engine.sql_engine import get_session
from lumen.db.enums import Permission
from lumen.db.models import User
from lumen.db.notification import (
    dismiss_all_notifications,
    get_notifications,
    update_notification_last_shown,
)
from lumen.error_handling.error_codes import LumenErrorCode
from lumen.error_handling.exceptions import LumenError
from lumen.key_value_store.factory import get_kv_store
from lumen.key_value_store.interface import KvKeyNotFoundError
from lumen.server.features.build.utils import (
    is_craft_available_for_deployment,
    is_craft_enabled_for_user,
)
from lumen.server.features.notifications.models import NotificationResponse
from lumen.server.settings.models import (
    DEFAULT_FILE_TOKEN_COUNT_THRESHOLD_K_NO_VECTOR_DB,
    DEFAULT_FILE_TOKEN_COUNT_THRESHOLD_K_VECTOR_DB,
    Settings,
    Tier,
    UserSettings,
)
from lumen.server.settings.store import (
    load_settings,
    settings_write_lock,
    store_settings,
)
from lumen.server.settings.tier_order import tier_at_least
from lumen.utils.audit import (
    AuditAction,
    AuditOutcome,
    actor_from_user,
    emit_audit_event,
)
from lumen.utils.logger import setup_logger
from lumen.utils.platform_utils import is_running_in_container
from lumen.utils.variable_functionality import (
    fetch_versioned_implementation_with_fallback,
    global_version,
)
from shared_configs.configs import MULTI_TENANT

logger = setup_logger()

admin_router = APIRouter(prefix="/admin/settings")
basic_router = APIRouter(prefix="/settings")


@admin_router.patch("")
def admin_patch_settings(
    settings: Settings,
    current_user: User = Depends(
        require_permission(Permission.FULL_ADMIN_PANEL_ACCESS)
    ),
) -> Settings:
    # Serialize the read-modify-write so two concurrent partial patches cannot
    # each merge onto a stale snapshot and drop the other's field.
    with settings_write_lock():
        # Fail closed: a settings-read error must not fall back to defaults and
        # silently disable an access-control field like invite_only_enabled.
        existing = load_settings(raise_on_error=True)
        # Merge only the fields the caller sent so omitted fields keep their
        # stored value. Access-control fields rely on this: a partial write
        # must not silently reset them to pydantic defaults.
        merged = existing.model_copy(
            update={
                field: getattr(settings, field)  # ods: ignore[getattr]
                for field in settings.model_fields_set
            }
        )

        if (
            merged.user_file_max_upload_size_mb is not None
            and merged.user_file_max_upload_size_mb > 0
            and merged.user_file_max_upload_size_mb > MAX_ALLOWED_UPLOAD_SIZE_MB
        ):
            raise LumenError(
                LumenErrorCode.INVALID_INPUT,
                f"File upload size limit cannot exceed {MAX_ALLOWED_UPLOAD_SIZE_MB} MB",
            )

        store_settings(merged)

        if merged.craft_default_enabled != existing.craft_default_enabled:
            emit_audit_event(
                AuditAction.CRAFT_DEFAULT_CHANGE,
                AuditOutcome.SUCCESS,
                actor=actor_from_user(current_user),
                resource_type="settings",
                extra={"craft_default_enabled": merged.craft_default_enabled},
            )

        # Read back rather than returning `merged`, so the response matches what
        # the next GET reports: the store clamps some values and env vars
        # override others. Saves every caller a second round trip.
        return load_settings(raise_on_error=True)


def apply_license_status_to_settings(settings: Settings) -> Settings:
    """Community build: the ported features (user groups, group sharing,
    enterprise settings/branding) are shipped unconditionally, so the frontend
    should treat EE-gated UI as available. No license check exists."""
    settings.ee_features_enabled = True
    return settings


@basic_router.get("")
def fetch_settings(
    user: User = Depends(
        require_permission(Permission.BASIC_ACCESS, allow_anonymous=True)
    ),
    db_session: Session = Depends(get_session),
) -> UserSettings:
    """Settings and notifications are stuffed into this single endpoint to reduce number of
    Postgres calls"""
    general_settings = load_settings()
    settings_notifications = get_settings_notifications(user, db_session)

    try:
        kv_store = get_kv_store()
        needs_reindexing = cast(bool, kv_store.load(KV_REINDEX_KEY))
    except KvKeyNotFoundError:
        needs_reindexing = False

    apply_fn = fetch_versioned_implementation_with_fallback(
        "lumen.server.settings.api",
        "apply_license_status_to_settings",
        apply_license_status_to_settings,
    )
    general_settings = apply_fn(general_settings)

    # Craft workspace instructions are visible to authenticated users (they
    # appear in sandbox AGENTS.md anyway) but not to anonymous visitors.
    if user is None:
        general_settings.craft_instructions = None

    # Check if Lumen Craft is enabled for this user (used for server-side
    # redirects). The deployment gate and already-loaded settings are shared.
    lumen_craft_available = is_craft_available_for_deployment(user) if user else False
    lumen_craft_enabled_for_user = (
        is_craft_enabled_for_user(
            user,
            deployment_available=lumen_craft_available,
            workspace_default=general_settings.craft_default_enabled,
        )
        if user
        else False
    )

    # Dev/debug flag: tail-the-pod-logs button gated by an env var. Same
    # check happens on the SSE endpoint so flipping the env var off
    # immediately closes the surface, not just the UI.
    from lumen.server.features.build.configs import ENABLE_OPENCODE_DEBUGGING

    return UserSettings(
        **general_settings.model_dump(),
        notifications=settings_notifications,
        needs_reindexing=needs_reindexing,
        lumen_craft_enabled=lumen_craft_enabled_for_user,
        lumen_craft_available=lumen_craft_available,
        opencode_debugging_enabled=ENABLE_OPENCODE_DEBUGGING,
        vector_db_enabled=not DISABLE_VECTOR_DB,
        hooks_enabled=not MULTI_TENANT,
        version=lumen_version,
        max_allowed_upload_size_mb=MAX_ALLOWED_UPLOAD_SIZE_MB,
        default_user_file_max_upload_size_mb=min(
            DEFAULT_USER_FILE_MAX_UPLOAD_SIZE_MB,
            MAX_ALLOWED_UPLOAD_SIZE_MB,
        ),
        default_file_token_count_threshold_k=(
            DEFAULT_FILE_TOKEN_COUNT_THRESHOLD_K_NO_VECTOR_DB
            if DISABLE_VECTOR_DB
            else DEFAULT_FILE_TOKEN_COUNT_THRESHOLD_K_VECTOR_DB
        ),
        is_containerized=is_running_in_container(),
        posthog_key=POSTHOG_API_KEY,
        posthog_host=POSTHOG_HOST,
    )


def get_settings_notifications(
    user: User, db_session: Session
) -> list[NotificationResponse]:
    """Get notifications for settings page, including product gating and reindex notifications"""
    # Check for product gating notification
    product_notif = get_notifications(
        user=None,
        notif_type=NotificationType.TRIAL_ENDS_TWO_DAYS,
        db_session=db_session,
    )
    notifications = (
        [NotificationResponse.model_validate(product_notif[0])] if product_notif else []
    )

    # Only show reindex notifications to admins
    if not is_user_admin(user):
        return notifications

    # Check if reindexing is needed
    kv_store = get_kv_store()
    try:
        needs_index = cast(bool, kv_store.load(KV_REINDEX_KEY))
        if not needs_index:
            dismiss_all_notifications(
                notif_type=NotificationType.REINDEX, db_session=db_session
            )
            return notifications
    except KvKeyNotFoundError:
        # If something goes wrong and the flag is gone, better to not start a reindexing
        # it's a heavyweight long running job and maybe this flag is cleaned up later
        logger.warning("Could not find reindex flag")
        return notifications

    try:
        # Need a transaction in order to prevent under-counting current notifications
        reindex_notifs = get_notifications(
            user=user, notif_type=NotificationType.REINDEX, db_session=db_session
        )

        if len(reindex_notifs) > 1:
            logger.error("User has multiple reindex notifications")
        elif not reindex_notifs:
            return notifications

        reindex_notif = reindex_notifs[0]
        update_notification_last_shown(
            notification=reindex_notif, db_session=db_session
        )

        db_session.commit()
        notifications.append(NotificationResponse.model_validate(reindex_notif))
        return notifications
    except SQLAlchemyError:
        logger.exception("Error while processing notifications")
        db_session.rollback()
        return notifications
