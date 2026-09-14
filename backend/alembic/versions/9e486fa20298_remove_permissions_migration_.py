"""remove permissions migration notification

Revision ID: 9e486fa20298
Revises: 484a6556a784
Create Date: 2026-09-13 16:03:46.272570

"""
from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision = '9e486fa20298'
down_revision = '484a6556a784'
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.execute(
        sa.text(
            "DELETE FROM notification "
            "WHERE notif_type = 'FEATURE_ANNOUNCEMENT' "
            "AND additional_data ->> 'feature' = 'permissions_migration_v1'"
        )
    )


def downgrade() -> None:
    pass
