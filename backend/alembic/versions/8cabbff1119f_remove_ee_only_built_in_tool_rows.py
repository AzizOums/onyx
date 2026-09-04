"""remove ee-only built-in tool rows

Revision ID: 8cabbff1119f
Revises: 287021f3b46c
Create Date: 2026-09-04 14:33:57.381942

"""
from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision = "8cabbff1119f"
down_revision = "287021f3b46c"
branch_labels = None
depends_on = None

# Built-in tool rows whose backing tool classes are not shipped in this
# Community-only build. They were seeded by earlier migrations but the tool
# constructor skips them, so the rows are dead weight exposed to the UI.
EE_ONLY_TOOL_IDS = ("PythonTool", "CodingAgentTool", "OktaProfileTool")


def upgrade() -> None:
    conn = op.get_bind()
    # persona__tool.tool_id FK -> tool.id must be cleared first.
    conn.execute(
        sa.text(
            "DELETE FROM persona__tool WHERE tool_id IN "
            "(SELECT id FROM tool WHERE in_code_tool_id = ANY(:ids))"
        ),
        {"ids": list(EE_ONLY_TOOL_IDS)},
    )
    conn.execute(
        sa.text("DELETE FROM tool WHERE in_code_tool_id = ANY(:ids)"),
        {"ids": list(EE_ONLY_TOOL_IDS)},
    )


def downgrade() -> None:
    # Rows are re-created by no live code path; restoring them is a no-op.
    pass
