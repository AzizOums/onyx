import json
from typing import Any

from fastapi import APIRouter, Depends, Request
from fastapi.concurrency import run_in_threadpool
from pydantic import ValidationError

from lumen.auth.permissions import require_permission
from lumen.auth.sso_url_guard import UnsafeSSOUrl, validate_idp_url
from lumen.db.enums import Permission
from lumen.db.models import User
from lumen.error_handling.error_codes import LumenErrorCode
from lumen.error_handling.exceptions import LumenError
from lumen.server.security.models import (
    OPERATOR_LOCKED_FIELDS,
    SecuritySettings,
    SecuritySettingsOverrides,
)
from lumen.server.security.store import (
    apply_patch,
    env_pinned_active_fields,
    get_security_settings,
)
from lumen.utils.audit import AuditActor
from lumen.utils.logger import setup_logger
from shared_configs.configs import MULTI_TENANT

logger = setup_logger()

admin_router = APIRouter(prefix="/admin/security")

# Single-tenant only, reject in multi-tenant where it would never enforce.
_PASSWORD_LOCKDOWN_FIELDS = frozenset({"password_auth_enabled"})


def _parse_put_body(raw: bytes) -> tuple[SecuritySettingsOverrides, set[str]]:
    """Parse the PUT body. Returns (parsed model, present_keys).

    ``present_keys`` is the set of keys the caller actually included — needed
    downstream to distinguish absent (keep existing) from explicit-null (clear
    → env fallback), which Pydantic collapses to ``None`` on the model.
    """
    try:
        payload_dict = json.loads(raw) if raw else {}
    except json.JSONDecodeError as e:
        raise LumenError(LumenErrorCode.INVALID_INPUT, f"Malformed JSON: {e}")

    if not isinstance(payload_dict, dict):
        raise LumenError(LumenErrorCode.INVALID_INPUT, "Body must be a JSON object")

    try:
        overrides = SecuritySettingsOverrides.model_validate(payload_dict)
    except ValidationError as e:
        raise LumenError(LumenErrorCode.INVALID_INPUT, str(e))

    payload_dict_typed: dict[str, Any] = payload_dict
    return overrides, set(payload_dict_typed.keys())


@admin_router.get("")
def get_security_settings_endpoint(
    _: User = Depends(require_permission(Permission.FULL_ADMIN_PANEL_ACCESS)),
) -> SecuritySettings:
    return get_security_settings()


@admin_router.get("/pinned-fields")
def get_pinned_fields_endpoint(
    _: User = Depends(require_permission(Permission.FULL_ADMIN_PANEL_ACCESS)),
) -> list[str]:
    """Fields currently pinned by env vars, so the UI can lock their inputs."""
    return sorted(env_pinned_active_fields())


@admin_router.put("")
async def put_security_settings_endpoint(
    request: Request,
    user: User = Depends(require_permission(Permission.FULL_ADMIN_PANEL_ACCESS)),
) -> SecuritySettings:
    raw = await request.body()
    overrides, present_keys = _parse_put_body(raw)

    if "jwt_public_key_url" in present_keys and overrides.jwt_public_key_url:
        try:
            validate_idp_url(overrides.jwt_public_key_url, field="jwt_public_key_url")
        except UnsafeSSOUrl as e:
            raise LumenError(LumenErrorCode.INVALID_INPUT, str(e))

    # A clear refusal instead of silently storing an override the env pin
    # would render inert.
    pinned_in_payload = present_keys & env_pinned_active_fields()
    if pinned_in_payload:
        raise LumenError(
            LumenErrorCode.INSUFFICIENT_PERMISSIONS,
            "These fields are pinned by environment variables on this deployment: "
            + ", ".join(sorted(pinned_in_payload)),
        )

    lockdown_in_payload = present_keys & _PASSWORD_LOCKDOWN_FIELDS
    if lockdown_in_payload and MULTI_TENANT:
        raise LumenError(
            LumenErrorCode.INVALID_INPUT,
            "Password login controls apply only to password-based deployments: "
            + ", ".join(sorted(lockdown_in_payload)),
        )

    # Primary boundary for operator-locked fields. The storage layer also
    # strips them; this gives admins a clear 403 instead of a silent no-op.
    if MULTI_TENANT:
        locked_in_payload = present_keys & OPERATOR_LOCKED_FIELDS
        if locked_in_payload:
            raise LumenError(
                LumenErrorCode.INSUFFICIENT_PERMISSIONS,
                "These fields are operator-controlled in multi-tenant deployments: "
                + ", ".join(sorted(locked_in_payload)),
            )

    # The auth dependency always yields a user, test seams may not.
    actor = (
        AuditActor(user_id=str(user.id), email=user.email) if user is not None else None
    )
    # Sync DB + Redis IO — offload so the event loop isn't blocked.
    return await run_in_threadpool(apply_patch, overrides, present_keys, actor)
