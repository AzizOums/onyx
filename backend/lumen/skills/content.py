"""Helpers for reading skill source content."""

from lumen.db.models import Skill
from lumen.error_handling.error_codes import LumenErrorCode
from lumen.error_handling.exceptions import LumenError
from lumen.file_store.file_store import FileStore, get_default_file_store
from lumen.skills.built_in import BuiltInSkillDefinition
from lumen.skills.bundle import (
    SKILL_MD_NAME,
    TEMPLATE_SUFFIX,
    read_custom_bundle_instructions,
    strip_skill_md_frontmatter,
)


def read_builtin_skill_instructions(definition: BuiltInSkillDefinition) -> str:
    source_path = definition.source_dir / SKILL_MD_NAME
    if not source_path.is_file():
        source_path = definition.source_dir / f"{SKILL_MD_NAME}{TEMPLATE_SUFFIX}"
    if not source_path.is_file():
        raise LumenError(
            LumenErrorCode.INTERNAL_ERROR,
            f"Built-in skill '{definition.built_in_skill_id}' has no SKILL.md source.",
        )
    try:
        return strip_skill_md_frontmatter(source_path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise LumenError(
            LumenErrorCode.INTERNAL_ERROR,
            f"Failed to read built-in skill '{definition.built_in_skill_id}'.",
        ) from exc


def read_custom_skill_bundle_instructions(
    skill: Skill,
    file_store: FileStore | None = None,
) -> str:
    bundle_bytes = read_custom_skill_bundle_bytes(skill, file_store)
    return read_custom_bundle_instructions(bundle_bytes)


def read_custom_skill_bundle_bytes(
    skill: Skill,
    file_store: FileStore | None = None,
) -> bytes:
    if skill.bundle_file_id is None:
        raise LumenError(
            LumenErrorCode.INTERNAL_ERROR,
            f"Custom skill '{skill.name}' has no bundle.",
        )
    store = file_store or get_default_file_store()
    try:
        bundle_bytes = store.read_file(skill.bundle_file_id).read()
    except Exception as exc:
        raise LumenError(
            LumenErrorCode.INTERNAL_ERROR,
            f"Failed to read bundle for skill '{skill.name}'.",
        ) from exc
    return bundle_bytes
