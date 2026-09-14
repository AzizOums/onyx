#!/usr/bin/env python3
"""Generate the Rust row structs from the SQLAlchemy models.

The SQLAlchemy models stay the single source of truth for the schema. This
writes the Rust side; `backend/tests/unit/lumen/db/test_generated_models.py`
fails when the committed file drifts from the models, so a migration cannot
quietly desynchronise the two.

An unmapped column type is a hard error rather than a guess: a silently wrong
type is the failure mode this whole approach exists to prevent.

Run from the repo root:

    uv run python backend/native/lumen-db/scripts/generate_models.py
"""

from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import Any

# Same idiom as backend/scripts/*.py: put `backend/` on the path.
_CRATE_DIR = Path(os.path.abspath(__file__)).parent.parent
sys.path.append(str(_CRATE_DIR.parent.parent))

OUTPUT_PATH = _CRATE_DIR / "src" / "generated.rs"

# The first slice: what sandbox_proxy reads and writes. Widen deliberately —
# every name added here is a schema the Rust side must then keep in step with.
MODELS: list[str] = [
    "ActionApproval",
    "BuildSession",
    "ExternalApp",
    "GatedApp",
    "MCPServer",
    "Notification",
    "Sandbox",
    "User",
]

# SQLAlchemy type class -> Rust type. Enums and arrays are handled separately.
SCALAR_TYPES: dict[str, str] = {
    "String": "String",
    "Text": "String",
    "VARCHAR": "String",
    "Boolean": "bool",
    "Integer": "i32",
    "SmallInteger": "i16",
    "BigInteger": "i64",
    "Float": "f64",
    "DateTime": "chrono::NaiveDateTime",
    "TIMESTAMPAware": "chrono::DateTime<chrono::Utc>",
    "Date": "chrono::NaiveDate",
    "UUID": "uuid::Uuid",
    "GUID": "uuid::Uuid",
    "JSONB": "serde_json::Value",
    "JSON": "serde_json::Value",
    "LargeBinary": "Vec<u8>",
    # Ciphertext. A newtype keeps it from being mistaken for readable text: the
    # Rust side has no decryption key.
    "EncryptedString": "crate::Encrypted",
    "EncryptedJson": "crate::Encrypted",
}

RESERVED = {
    "type",
    "ref",
    "match",
    "move",
    "box",
    "fn",
    "mod",
    "use",
    "impl",
    "self",
    "crate",
    "super",
    "where",
    "async",
    "await",
    "loop",
    "final",
    "override",
}


class UnmappedType(RuntimeError):
    """A column type the generator does not know how to render."""


def rust_field_name(column_name: str) -> str:
    """Escape a column name that collides with a Rust keyword."""
    return f"r#{column_name}" if column_name in RESERVED else column_name


def enum_type_name(model: str, column: str) -> str:
    """Rust name for a column-level enum, e.g. sandbox.status -> SandboxStatus."""
    parts = "".join(piece.capitalize() for piece in column.split("_"))
    return f"{model}{parts}"


def rust_scalar(column: Any, model: str) -> tuple[str, dict | None]:
    """Return the Rust type for one column, plus an enum definition if it makes one."""
    type_name = type(column.type).__name__

    if type_name == "Enum":
        values = list(column.type.enums or [])
        if not values:
            raise UnmappedType(f"{model}.{column.name}: Enum with no values")
        name = enum_type_name(model, column.name)
        return name, {"name": name, "values": values}

    if type_name == "ARRAY":
        inner_name = type(column.type.item_type).__name__
        inner = SCALAR_TYPES.get(inner_name)
        if inner is None:
            raise UnmappedType(
                f"{model}.{column.name}: array of unmapped type {inner_name}"
            )
        return f"Vec<{inner}>", None

    mapped = SCALAR_TYPES.get(type_name)
    if mapped is None:
        raise UnmappedType(
            f"{model}.{column.name}: unmapped column type {type_name}. "
            f"Add it to SCALAR_TYPES in {Path(__file__).name} once you are sure "
            "of the Rust representation."
        )
    return mapped, None


def collect() -> tuple[list[dict], list[dict]]:
    """Describe every requested model and the enums its columns imply."""
    from lumen.db import models as db_models

    described: list[dict] = []
    enums: dict[str, dict] = {}

    for model_name in MODELS:
        model = getattr(db_models, model_name, None)
        if model is None:
            raise RuntimeError(f"{model_name} is not defined in lumen.db.models")

        table = model.__table__
        fields = []
        for column in table.columns:
            rust_type, enum_def = rust_scalar(column, model_name)
            if enum_def is not None:
                existing = enums.get(enum_def["name"])
                if existing and existing["values"] != enum_def["values"]:
                    raise RuntimeError(
                        f"two different enums generated the name {enum_def['name']}"
                    )
                enums[enum_def["name"]] = enum_def
            fields.append(
                {
                    "column": column.name,
                    "rust_type": rust_type,
                    "nullable": bool(column.nullable),
                    "primary_key": bool(column.primary_key),
                }
            )

        described.append({"model": model_name, "table": table.name, "fields": fields})

    return described, [enums[name] for name in sorted(enums)]


def render(models: list[dict], enums: list[dict]) -> str:
    lines: list[str] = [
        "//! Row structs generated from the SQLAlchemy models. Do not edit.",
        "//!",
        "//! Regenerate with:",
        "//!",
        "//! ```text",
        "//! uv run python backend/native/lumen-db/scripts/generate_models.py",
        "//! ```",
        "//!",
        "//! `backend/tests/unit/lumen/db/test_generated_models.py` fails when this",
        "//! file drifts from `backend/lumen/db/models.py`.",
        "",
        "#![allow(clippy::struct_field_names)]",
        "",
        "use serde::{Deserialize, Serialize};",
        "",
    ]

    for enum in enums:
        lines.append(
            "/// Stored as text; the variants are the values the column accepts."
        )
        lines.append(
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]"
        )
        lines.append(f"pub enum {enum['name']} {{")
        for value in enum["values"]:
            variant = "".join(piece.capitalize() for piece in value.split("_"))
            lines.append(f'    #[serde(rename = "{value}")]')
            lines.append(f"    {variant},")
        lines.append("}")
        lines.append("")
        lines.append(f"impl {enum['name']} {{")
        lines.append("    pub const fn as_str(self) -> &'static str {")
        lines.append("        match self {")
        for value in enum["values"]:
            variant = "".join(piece.capitalize() for piece in value.split("_"))
            lines.append(f'            Self::{variant} => "{value}",')
        lines.append("        }")
        lines.append("    }")
        lines.append("}")
        lines.append("")
        lines.append(f"impl std::str::FromStr for {enum['name']} {{")
        lines.append("    type Err = crate::UnknownEnumValue;")
        lines.append("")
        lines.append("    fn from_str(value: &str) -> Result<Self, Self::Err> {")
        lines.append("        match value {")
        for value in enum["values"]:
            variant = "".join(piece.capitalize() for piece in value.split("_"))
            lines.append(f'            "{value}" => Ok(Self::{variant}),')
        lines.append("            other => Err(crate::UnknownEnumValue {")
        lines.append(f'                enum_name: "{enum["name"]}",')
        lines.append("                value: other.to_string(),")
        lines.append("            }),")
        lines.append("        }")
        lines.append("    }")
        lines.append("}")
        lines.append("")

    for model in models:
        primary = [f["column"] for f in model["fields"] if f["primary_key"]]
        lines.append(f"/// Row of `{model['table']}`.")
        if primary:
            lines.append(f"/// Primary key: {', '.join(primary)}.")
        lines.append("#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]")
        lines.append(f"pub struct {model['model']} {{")
        for field in model["fields"]:
            rust_type = field["rust_type"]
            if field["nullable"]:
                rust_type = f"Option<{rust_type}>"
            lines.append(f"    pub {rust_field_name(field['column'])}: {rust_type},")
        lines.append("}")
        lines.append("")
        lines.append(f"impl {model['model']} {{")
        lines.append(f'    pub const TABLE: &\'static str = "{model["table"]}";')
        lines.append("    pub const COLUMNS: &'static [&'static str] = &[")
        lines.extend(f'        "{field["column"]}",' for field in model["fields"])
        lines.append("    ];")
        lines.append("}")
        lines.append("")

    return "\n".join(lines).rstrip() + "\n"


def main() -> int:
    models, enums = collect()
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_text(render(models, enums), encoding="utf-8")
    columns = sum(len(model["fields"]) for model in models)
    print(
        f"wrote {len(models)} models ({columns} columns) and {len(enums)} enums "
        f"to {OUTPUT_PATH}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
