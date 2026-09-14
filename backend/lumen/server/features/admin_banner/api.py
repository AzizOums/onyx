from fastapi import APIRouter, Depends
from pydantic import BaseModel, Field
from sqlalchemy.orm import Session

from lumen.auth.permissions import require_permission
from lumen.configs.constants import NotificationType
from lumen.db.admin_banner import (
    AdminBanner,
    clear_admin_banner,
    get_admin_banner,
    set_admin_banner,
)
from lumen.db.engine.sql_engine import get_session
from lumen.db.enums import Permission
from lumen.db.models import User
from lumen.db.notification import delete_notifications_by_type
from lumen.error_handling.error_codes import LumenErrorCode
from lumen.error_handling.exceptions import LumenError

MAX_TITLE_LEN = 100
MAX_CONTENT_LEN = 1000


class AdminBannerUpdateRequest(BaseModel):
    title: str = Field(..., max_length=MAX_TITLE_LEN)
    content: str | None = Field(default=None, max_length=MAX_CONTENT_LEN)
    show_as_popup: bool = False


# Admin-only configuration of the single site-wide banner. Users receive it
# through the notification feed (synthesized per-user on read), not a dedicated
# display endpoint, so publish/clear here wipes all SYSTEM_ANNOUNCEMENT rows.
admin_router = APIRouter(prefix="/admin/banner")


@admin_router.get("")
def get_admin_banner_config(
    _: User = Depends(require_permission(Permission.FULL_ADMIN_PANEL_ACCESS)),
) -> AdminBanner | None:
    return get_admin_banner()


@admin_router.put("")
def upsert_admin_banner(
    request: AdminBannerUpdateRequest,
    _: User = Depends(require_permission(Permission.FULL_ADMIN_PANEL_ACCESS)),
    db_session: Session = Depends(get_session),
) -> AdminBanner:
    title = request.title.strip()
    if not title:
        raise LumenError(
            LumenErrorCode.INVALID_INPUT,
            "Title must include non-whitespace characters",
        )
    content = (request.content or "").strip() or None
    banner = set_admin_banner(
        title=title, content=content, show_as_popup=request.show_as_popup
    )
    # Clear existing rows so every user re-materializes the edited banner.
    delete_notifications_by_type(NotificationType.SYSTEM_ANNOUNCEMENT, db_session)
    return banner


@admin_router.delete("", status_code=204)
def delete_admin_banner(
    _: User = Depends(require_permission(Permission.FULL_ADMIN_PANEL_ACCESS)),
    db_session: Session = Depends(get_session),
) -> None:
    clear_admin_banner()
    delete_notifications_by_type(NotificationType.SYSTEM_ANNOUNCEMENT, db_session)
