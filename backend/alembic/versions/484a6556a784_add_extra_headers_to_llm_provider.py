"""add extra_headers to llm_provider

Revision ID: 484a6556a784
Revises: 8cabbff1119f
Create Date: 2026-09-12 03:46:17.897710

"""

from alembic import op
import sqlalchemy as sa
from sqlalchemy.dialects import postgresql


# revision identifiers, used by Alembic.
revision = "484a6556a784"
down_revision = "8cabbff1119f"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.add_column(
        "llm_provider", sa.Column("extra_headers", postgresql.JSONB(), nullable=True)
    )


def downgrade() -> None:
    op.drop_column("llm_provider", "extra_headers")
