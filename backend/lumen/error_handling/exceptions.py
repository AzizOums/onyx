"""LumenError — the single exception type for all Lumen business errors.

Raise ``LumenError`` instead of ``HTTPException`` in business code.  A global
FastAPI exception handler (registered via ``register_lumen_exception_handlers``)
converts it into a JSON response with the standard
``{"error_code": "...", "detail": "..."}`` shape.

Usage::

    from lumen.error_handling.error_codes import LumenErrorCode
    from lumen.error_handling.exceptions import LumenError

    raise LumenError(LumenErrorCode.NOT_FOUND, "Session not found")

For upstream errors with a dynamic HTTP status (e.g. billing service),
use ``status_code_override``::

    raise LumenError(
        LumenErrorCode.BAD_GATEWAY,
        detail,
        status_code_override=upstream_status,
    )
"""

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse

from lumen.error_handling.error_codes import LumenErrorCode
from lumen.utils.logger import setup_logger

logger = setup_logger()


class LumenError(Exception):
    """Structured error that maps to a specific ``LumenErrorCode``.

    Attributes:
        error_code: The ``LumenErrorCode`` enum member.
        detail: Human-readable detail (defaults to the error code string).
        status_code: HTTP status — either overridden or from the error code.
    """

    def __init__(
        self,
        error_code: LumenErrorCode,
        detail: str | None = None,
        *,
        status_code_override: int | None = None,
        extra: dict[str, object] | None = None,
        headers: dict[str, str] | None = None,
    ) -> None:
        resolved_detail = detail or error_code.code
        super().__init__(resolved_detail)
        self.error_code = error_code
        self.detail = resolved_detail
        self._status_code_override = status_code_override
        # extra: machine-readable fields merged into the JSON body (e.g. reset_at).
        # headers: response headers the FE/clients need (e.g. Retry-After).
        self.extra = extra
        self.headers = headers

    @property
    def status_code(self) -> int:
        return self._status_code_override or self.error_code.status_code


def log_lumen_error(exc: LumenError) -> None:
    detail = exc.detail
    status_code = exc.status_code
    if status_code >= 500:
        logger.error("LumenError %s: %s", exc.error_code.code, detail)
    elif status_code >= 400:
        logger.warning("LumenError %s: %s", exc.error_code.code, detail)


def lumen_error_to_json_response(exc: LumenError) -> JSONResponse:
    content = exc.error_code.detail(exc.detail)
    if exc.extra:
        # extra first so the canonical error_code/detail can't be overwritten.
        content = {**exc.extra, **content}
    return JSONResponse(
        status_code=exc.status_code,
        content=content,
        headers=exc.headers,
    )


def register_lumen_exception_handlers(app: FastAPI) -> None:
    """Register a global handler that converts ``LumenError`` to JSON responses.

    Must be called *after* the app is created but *before* it starts serving.
    The handler logs at WARNING for 4xx and ERROR for 5xx.
    """

    @app.exception_handler(LumenError)
    async def _handle_lumen_error(
        request: Request,  # noqa: ARG001
        exc: LumenError,
    ) -> JSONResponse:
        log_lumen_error(exc)
        return lumen_error_to_json_response(exc)
