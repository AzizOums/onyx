"""The Rust row structs are generated from these models; they must stay in step.

A Rust schema maintained by hand alongside 447 Alembic revisions would drift,
and a drifted schema corrupts data without raising anything. This test is what
makes the generated copy safe to rely on: it regenerates in memory and compares.
"""

import importlib.util
import sys
from pathlib import Path

_CRATE_DIR = Path(__file__).resolve().parents[4] / "native" / "lumen-db"
_GENERATOR_PATH = _CRATE_DIR / "scripts" / "generate_models.py"
_GENERATED_PATH = _CRATE_DIR / "src" / "generated.rs"
_REGENERATE = f"Regenerate it with: uv run python {_GENERATOR_PATH.relative_to(Path(__file__).resolve().parents[5])}"


def _load_generator():
    spec = importlib.util.spec_from_file_location("lumen_db_generator", _GENERATOR_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_generated_rust_matches_the_models() -> None:
    assert _GENERATED_PATH.is_file(), f"{_GENERATED_PATH} is missing. {_REGENERATE}"

    generator = _load_generator()
    models, enums = generator.collect()
    expected = generator.render(models, enums)

    actual = _GENERATED_PATH.read_text(encoding="utf-8")
    assert actual == expected, (
        f"The Rust row structs are stale against lumen/db/models.py. {_REGENERATE}"
    )


def test_every_requested_model_still_exists() -> None:
    """A renamed or dropped model must fail here, not at runtime in Rust."""
    generator = _load_generator()
    from lumen.db import models as db_models

    missing = [name for name in generator.MODELS if not hasattr(db_models, name)]
    assert not missing, (
        f"generate_models.py lists models that no longer exist: {missing}"
    )


def test_an_unmapped_column_type_is_refused() -> None:
    """The generator must never guess a Rust type it does not know."""
    generator = _load_generator()

    class _Unknown:
        pass

    class _Column:
        name = "mystery"
        type = _Unknown()

    try:
        generator.rust_scalar(_Column(), "Fake")
    except generator.UnmappedType as error:
        assert "unmapped column type" in str(error)
    else:
        raise AssertionError("an unknown column type must raise, not be guessed")
