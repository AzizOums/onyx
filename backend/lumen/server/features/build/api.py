from fastapi import APIRouter, Depends

from lumen.auth.permissions import require_permission
from lumen.db.enums import Permission
from lumen.db.models import User
from lumen.error_handling.error_codes import LumenErrorCode
from lumen.error_handling.exceptions import LumenError
from lumen.server.features.build.approvals.api import router as approvals_router
from lumen.server.features.build.debug import router as debug_router
from lumen.server.features.build.external_apps.api import (
    admin_router as external_apps_admin_router,
)
from lumen.server.features.build.external_apps.api import router as external_apps_router
from lumen.server.features.build.external_apps.oauth import (
    router as external_apps_oauth_router,
)
from lumen.server.features.build.interactive_turns.api import router as turns_router
from lumen.server.features.build.models import BaseInstructionsResponse
from lumen.server.features.build.sandbox.util.agent_instructions import (
    AGENT_INSTRUCTIONS_TEMPLATE_PATH,
)
from lumen.server.features.build.scheduled_tasks.api import (
    router as scheduled_tasks_router,
)
from lumen.server.features.build.session.api import router as sessions_router
from lumen.server.features.build.session.messages import router as messages_router
from lumen.server.features.build.user_library.api import router as user_library_router
from lumen.server.features.build.utils import is_craft_enabled_for_user
from lumen.utils.logger import setup_logger

logger = setup_logger()


def require_lumen_craft_enabled(
    user: User = Depends(require_permission(Permission.BASIC_ACCESS)),
) -> User:
    if not is_craft_enabled_for_user(user):
        raise LumenError(
            LumenErrorCode.INSUFFICIENT_PERMISSIONS,
            "Lumen Craft is not available",
        )
    return user


router = APIRouter(prefix="/build", dependencies=[Depends(require_lumen_craft_enabled)])

# Admin-only Craft endpoints. Deliberately NOT behind the craft-enabled-for-user
# gate: an admin configuring Craft may not have Craft enabled for themselves.
admin_router = APIRouter(
    prefix="/build/admin",
    dependencies=[Depends(require_permission(Permission.FULL_ADMIN_PANEL_ACCESS))],
)
admin_router.include_router(external_apps_admin_router, tags=["build"])


@admin_router.get("/base-instructions")
def get_base_instructions(
    _: User = Depends(require_permission(Permission.FULL_ADMIN_PANEL_ACCESS)),
) -> BaseInstructionsResponse:
    """The base AGENTS.md template, so admins can see what their workspace
    instructions are appended to. Dynamic sections appear as placeholders."""
    if not AGENT_INSTRUCTIONS_TEMPLATE_PATH.exists():
        raise LumenError(LumenErrorCode.NOT_FOUND, "Base instructions not found")
    return BaseInstructionsResponse(
        content=AGENT_INSTRUCTIONS_TEMPLATE_PATH.read_text()
    )


router.include_router(sessions_router, tags=["build"])
router.include_router(messages_router, tags=["build"])
router.include_router(turns_router, tags=["build"])
router.include_router(user_library_router, tags=["build"])
router.include_router(scheduled_tasks_router, tags=["build"])
router.include_router(external_apps_router, tags=["build"])
router.include_router(external_apps_oauth_router, tags=["build"])
router.include_router(debug_router, tags=["build-debug"])
router.include_router(approvals_router, tags=["build"])
