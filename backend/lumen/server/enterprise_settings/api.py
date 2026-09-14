from datetime import datetime, timezone
from typing import Any

import httpx
from fastapi import APIRouter, Depends, HTTPException, Response, UploadFile, status
from pydantic import BaseModel, Field
from sqlalchemy.orm import Session

from lumen.server.enterprise_settings.models import (
    AnalyticsScriptUpload,
    EnterpriseSettings,
)
from lumen.server.enterprise_settings.store import (
    get_logo_filename,
    get_logotype_filename,
    load_analytics_script,
    load_settings,
    store_analytics_script,
    store_settings,
    upload_logo,
)
from lumen.auth.permissions import require_permission
from lumen.auth.users import (
    UserManager,
    current_user_with_expired_token,
    get_user_manager,
)
from lumen.db.engine.sql_engine import get_session
from lumen.db.enums import Permission
from lumen.db.models import User
from lumen.error_handling.error_codes import LumenErrorCode
from lumen.error_handling.exceptions import LumenError
from lumen.file_store.file_store import get_default_file_store
from lumen.server.utils import BasicAuthenticationError
from lumen.utils.logger import setup_logger
from shared_configs.configs import MULTI_TENANT, POSTGRES_DEFAULT_SCHEMA
from shared_configs.contextvars import get_current_tenant_id

admin_router = APIRouter(prefix="/admin/enterprise-settings")
basic_router = APIRouter(prefix="/enterprise-settings")

logger = setup_logger()


class RefreshTokenData(BaseModel):
    access_token: str
    refresh_token: str
    session: dict = Field(..., description="Contains session information")
    userinfo: dict = Field(..., description="Contains user information")

    def __init__(self, **data: Any) -> None:
        super().__init__(**data)
        if "exp" not in self.session:
            raise ValueError("'exp' must be set in the session dictionary")
        if "userId" not in self.userinfo or "email" not in self.userinfo:
            raise ValueError(
                "'userId' and 'email' must be set in the userinfo dictionary"
            )


@basic_router.post("/refresh-token")
async def refresh_access_token(
    refresh_token: RefreshTokenData,
    user: User = Depends(current_user_with_expired_token),
    user_manager: UserManager = Depends(get_user_manager),
) -> None:
    try:
        logger.debug("Received response from Meechum auth URL for user %s", user.id)

        # Extract new tokens
        new_access_token = refresh_token.access_token
        new_refresh_token = refresh_token.refresh_token

        new_expiry = datetime.fromtimestamp(
            refresh_token.session["exp"] / 1000, tz=timezone.utc
        )
        expires_at_timestamp = int(new_expiry.timestamp())

        logger.debug("Access token has been refreshed for user %s", user.id)

        await user_manager.oauth_callback(
            oauth_name="custom",
            access_token=new_access_token,
            account_id=refresh_token.userinfo["userId"],
            account_email=refresh_token.userinfo["email"],
            expires_at=expires_at_timestamp,
            refresh_token=new_refresh_token,
            associate_by_email=True,
        )

        logger.info("Successfully refreshed tokens for user %s", user.id)

    except httpx.HTTPStatusError as e:
        if e.response.status_code == 401:
            logger.warning("Full authentication required for user %s", user.id)
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Full authentication required",
            )
        logger.error(
            "HTTP error occurred while refreshing token for user %s: %s",
            user.id,
            str(e),
        )
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Failed to refresh token",
        )
    except Exception as e:
        logger.error(
            "Unexpected error occurred while refreshing token for user %s: %s",
            user.id,
            str(e),
        )
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="An unexpected error occurred",
        )


@admin_router.put("")
def admin_ee_put_settings(
    settings: EnterpriseSettings,
    _: User = Depends(require_permission(Permission.FULL_ADMIN_PANEL_ACCESS)),
) -> None:
    store_settings(settings)


@basic_router.get("")
def ee_fetch_settings(
    _: User = Depends(
        require_permission(Permission.BASIC_ACCESS, allow_anonymous=True)
    ),
) -> EnterpriseSettings:
    if MULTI_TENANT:
        tenant_id = get_current_tenant_id()
        if not tenant_id or tenant_id == POSTGRES_DEFAULT_SCHEMA:
            raise BasicAuthenticationError(detail="User must authenticate")

    return load_settings()


@admin_router.put("/logo")
def put_logo(
    file: UploadFile,
    is_logotype: bool = False,
    _: User = Depends(require_permission(Permission.FULL_ADMIN_PANEL_ACCESS)),
) -> None:
    upload_logo(file=file, is_logotype=is_logotype)


def fetch_logo_helper(db_session: Session) -> Response:  # noqa: ARG001
    try:
        file_store = get_default_file_store()
        lumen_file = file_store.get_file_with_mime_type(get_logo_filename())
        if not lumen_file:
            raise ValueError("get_lumen_file returned None!")
    except Exception:
        logger.exception("Faield to fetch logo file")
        raise HTTPException(
            status_code=404,
            detail="No logo file found",
        )
    else:
        return Response(
            content=lumen_file.data,
            media_type=lumen_file.mime_type,
            headers={"Cache-Control": "no-cache"},
        )


def fetch_logotype_helper(db_session: Session) -> Response:  # noqa: ARG001
    try:
        file_store = get_default_file_store()
        lumen_file = file_store.get_file_with_mime_type(get_logotype_filename())
        if not lumen_file:
            raise ValueError("get_lumen_file returned None!")
    except Exception:
        raise HTTPException(
            status_code=404,
            detail="No logotype file found",
        )
    else:
        return Response(content=lumen_file.data, media_type=lumen_file.mime_type)


@basic_router.get("/logotype")
def fetch_logotype(
    _: User = Depends(
        require_permission(Permission.BASIC_ACCESS, allow_anonymous=True)
    ),
    db_session: Session = Depends(get_session),
) -> Response:
    return fetch_logotype_helper(db_session)


@basic_router.get("/logo")
def fetch_logo(
    is_logotype: bool = False,
    _: User = Depends(
        require_permission(Permission.BASIC_ACCESS, allow_anonymous=True)
    ),
    db_session: Session = Depends(get_session),
) -> Response:
    if is_logotype:
        return fetch_logotype_helper(db_session)

    return fetch_logo_helper(db_session)


@admin_router.put("/custom-analytics-script")
def upload_custom_analytics_script(
    script_upload: AnalyticsScriptUpload,
    _: User = Depends(require_permission(Permission.FULL_ADMIN_PANEL_ACCESS)),
) -> None:
    try:
        store_analytics_script(script_upload)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))


@basic_router.get("/custom-analytics-script")
def fetch_custom_analytics_script(
    _: User = Depends(require_permission(Permission.BASIC_ACCESS)),
) -> str | None:
    if MULTI_TENANT:
        tenant_id = get_current_tenant_id()
        if not tenant_id or tenant_id == POSTGRES_DEFAULT_SCHEMA:
            # Pre-auth / anonymous page loads reach this route with no tenant
            # context resolved (the contextvar defaults to the public schema,
            # which has no per-tenant key_value_store). Such a visitor has no
            # custom analytics script by definition — return None rather than
            # letting the KV lookup 500 against public.key_value_store.
            return None

    return load_analytics_script()

