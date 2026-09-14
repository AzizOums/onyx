"""Loader for the optional Rust text extraction module.

The module lives in `backend/native/lumen_text` and is built separately (see
`backend/native/README.md`). It is never required: when it is absent or disabled,
every caller falls back to the pure-Python implementation.

The native path is opt-in because the Rust parser (html5ever) and the Python
parser (lxml) recover differently from malformed HTML. They agree byte-for-byte
on well-formed markup. See `backend/native/README.md` for the measured numbers.

Set `LUMEN_NATIVE_TEXT=true` to enable it.
"""

import os
from types import ModuleType

from lumen.utils.logger import setup_logger

logger = setup_logger()

NATIVE_TEXT_ENABLED = os.environ.get("LUMEN_NATIVE_TEXT", "").lower() == "true"

_native_module: ModuleType | None = None

if NATIVE_TEXT_ENABLED:
    try:
        import lumen_text_native

        _native_module = lumen_text_native
        logger.info(
            "Native text extraction enabled (lumen_text_native %s)",
            lumen_text_native.__version__,
        )
    except ImportError:
        logger.warning(
            "LUMEN_NATIVE_TEXT is set but lumen_text_native is not installed. "
            "Falling back to the Python implementation. "
            "Build it with backend/native/build.sh"
        )


def get_native() -> ModuleType | None:
    """Return the native module, or None when the Python path should be used.

    Callers read this per call so tests can toggle the backend with monkeypatch.
    """
    return _native_module


def set_native(module: ModuleType | None) -> None:
    """Force the backend, ignoring the environment flag.

    For the parity harness in `backend/native/parity`, which runs the same input
    through both paths. Production code reads the flag instead.
    """
    global _native_module
    _native_module = module
