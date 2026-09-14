"""Rebrand persisted identifiers

Renames the identifiers the previous brand left in the database so a
deployment created before the rebrand keeps working:

- ``chat_session.onyxbot_flow`` -> ``lumenbot_flow`` (and its index)
- the seeded placeholder user emails
- the API-key dummy email domain
- the key-value store settings keys

Every step is conditional, so the migration is a no-op on a database created
after the rebrand and safe to run twice.

Revision ID: 6ebd31e9394c
Revises: 9e486fa20298
Create Date: 2026-09-14
"""

from alembic import op
import sqlalchemy as sa

revision = "6ebd31e9394c"
down_revision = "9e486fa20298"
branch_labels = None
depends_on = None

OLD_INDEX = "ix_chat_session_user_id_onyxbot_flow_time_updated"
NEW_INDEX = "ix_chat_session_user_id_lumenbot_flow_time_updated"

_EMAILS = [
    ("anonymous@onyx.app", "anonymous@lumen.app"),
    ("no-auth-placeholder@onyx.app", "no-auth-placeholder@lumen.app"),
]
_KV_KEYS = [
    ("onyx_settings", "lumen_settings"),
    ("onyx_enterprise_settings", "lumen_enterprise_settings"),
]


def _column_exists(table: str, column: str) -> bool:
    bind = op.get_bind()
    return bool(
        bind.execute(
            sa.text(
                "SELECT 1 FROM information_schema.columns "
                "WHERE table_schema = current_schema() "
                "AND table_name = :t AND column_name = :c"
            ),
            {"t": table, "c": column},
        ).scalar()
    )


def _index_exists(name: str) -> bool:
    bind = op.get_bind()
    return bool(
        bind.execute(
            sa.text(
                "SELECT 1 FROM pg_indexes "
                "WHERE schemaname = current_schema() AND indexname = :n"
            ),
            {"n": name},
        ).scalar()
    )


def _rename(old_column: str, new_column: str, old_index: str, new_index: str) -> None:
    if _index_exists(old_index):
        op.execute(sa.text(f'ALTER INDEX "{old_index}" RENAME TO "{new_index}"'))
    if _column_exists("chat_session", old_column) and not _column_exists(
        "chat_session", new_column
    ):
        op.alter_column("chat_session", old_column, new_column_name=new_column)


def upgrade() -> None:
    _rename("onyxbot_flow", "lumenbot_flow", OLD_INDEX, NEW_INDEX)

    bind = op.get_bind()
    for old_email, new_email in _EMAILS:
        bind.execute(
            sa.text('UPDATE "user" SET email = :new WHERE email = :old'),
            {"new": new_email, "old": old_email},
        )
    # API-key users are recognised by their dummy email domain.
    bind.execute(
        sa.text(
            "UPDATE \"user\" SET email = replace(email, '@onyxapikey.ai', "
            "'@lumenapikey.ai') WHERE email LIKE '%@onyxapikey.ai'"
        )
    )
    for old_key, new_key in _KV_KEYS:
        bind.execute(
            sa.text(
                "UPDATE key_value_store SET key = :new "
                "WHERE key = :old AND NOT EXISTS "
                "(SELECT 1 FROM key_value_store WHERE key = :new)"
            ),
            {"new": new_key, "old": old_key},
        )


def downgrade() -> None:
    _rename("lumenbot_flow", "onyxbot_flow", NEW_INDEX, OLD_INDEX)

    bind = op.get_bind()
    for old_email, new_email in _EMAILS:
        bind.execute(
            sa.text('UPDATE "user" SET email = :old WHERE email = :new'),
            {"new": new_email, "old": old_email},
        )
    bind.execute(
        sa.text(
            "UPDATE \"user\" SET email = replace(email, '@lumenapikey.ai', "
            "'@onyxapikey.ai') WHERE email LIKE '%@lumenapikey.ai'"
        )
    )
    for old_key, new_key in _KV_KEYS:
        bind.execute(
            sa.text(
                "UPDATE key_value_store SET key = :old "
                "WHERE key = :new AND NOT EXISTS "
                "(SELECT 1 FROM key_value_store WHERE key = :old)"
            ),
            {"new": new_key, "old": old_key},
        )
