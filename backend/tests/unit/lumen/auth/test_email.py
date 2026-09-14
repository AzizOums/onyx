import pytest

from lumen.auth.email_utils import build_user_email_invite, send_email
from lumen.configs.constants import LUMEN_DEFAULT_APPLICATION_NAME
from lumen.db.engine.sql_engine import SqlEngine
from lumen.server.runtime.lumen_runtime import LumenRuntime


@pytest.mark.skip(
    reason="This sends real emails, so only run when you really want to test this!"
)
def test_send_user_email_invite() -> None:
    SqlEngine.init_engine(pool_size=20, max_overflow=5)

    application_name = LUMEN_DEFAULT_APPLICATION_NAME

    lumen_file = LumenRuntime.get_emailable_logo()

    subject = f"Invitation to Join {application_name} Organization"

    FROM_EMAIL = "noreply@lumen.app"
    TO_EMAIL = "support@lumen.app"
    text_content, html_content = build_user_email_invite(
        FROM_EMAIL, TO_EMAIL, LUMEN_DEFAULT_APPLICATION_NAME
    )

    send_email(
        TO_EMAIL,
        subject,
        html_content,
        text_content,
        mail_from=FROM_EMAIL,
        inline_png=("logo.png", lumen_file.data),
    )
