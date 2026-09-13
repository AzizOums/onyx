from sqlalchemy.orm import Session

from onyx.configs.constants import NotificationType
from onyx.db.admin_banner import get_admin_banner
from onyx.db.enums import NotificationSeverity
from onyx.db.models import User
from onyx.db.notification import create_notification


def ensure_system_announcement_notification(user: User, db_session: Session) -> None:
    """Materialize the admin-authored site-wide announcement as a per-user
    notification so it flows through the bell + banner queue with the normal
    per-user dismissal. Re-show on edit is driven by the admin API clearing the
    rows, so a repeat read within one banner just returns the existing row."""
    banner = get_admin_banner()
    if banner is None:
        return
    create_notification(
        user_id=user.id,
        notif_type=NotificationType.SYSTEM_ANNOUNCEMENT,
        db_session=db_session,
        title=banner.title,
        description=banner.content,
        # The stable empty object is this announcement's deduplication key.
        additional_data={},
        refresh_existing=False,
        # An admin-authored announcement exists to be a banner.
        severity=NotificationSeverity.WARNING,
    )
