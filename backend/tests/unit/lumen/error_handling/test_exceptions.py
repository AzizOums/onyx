"""Tests for LumenError and the global exception handler."""

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

from lumen.error_handling.error_codes import LumenErrorCode
from lumen.error_handling.exceptions import LumenError, register_lumen_exception_handlers


class TestLumenError:
    """Unit tests for LumenError construction and properties."""

    def test_basic_construction(self) -> None:
        err = LumenError(LumenErrorCode.NOT_FOUND, "Session not found")
        assert err.error_code is LumenErrorCode.NOT_FOUND
        assert err.detail == "Session not found"
        assert err.status_code == 404

    def test_message_defaults_to_code(self) -> None:
        err = LumenError(LumenErrorCode.UNAUTHENTICATED)
        assert err.detail == "UNAUTHENTICATED"
        assert str(err) == "UNAUTHENTICATED"

    def test_status_code_override(self) -> None:
        err = LumenError(
            LumenErrorCode.BAD_GATEWAY,
            "upstream failed",
            status_code_override=503,
        )
        assert err.status_code == 503
        # error_code still reports its own default
        assert err.error_code.status_code == 502

    def test_no_override_uses_error_code_status(self) -> None:
        err = LumenError(LumenErrorCode.RATE_LIMITED, "slow down")
        assert err.status_code == 429

    def test_is_exception(self) -> None:
        err = LumenError(LumenErrorCode.INTERNAL_ERROR)
        assert isinstance(err, Exception)


class TestExceptionHandler:
    """Integration test: LumenError → JSON response via FastAPI TestClient."""

    @pytest.fixture()
    def client(self) -> TestClient:
        app = FastAPI()
        register_lumen_exception_handlers(app)

        @app.get("/boom")
        def _boom() -> None:
            raise LumenError(LumenErrorCode.NOT_FOUND, "Thing not found")

        @app.get("/boom-override")
        def _boom_override() -> None:
            raise LumenError(
                LumenErrorCode.BAD_GATEWAY,
                "upstream 503",
                status_code_override=503,
            )

        @app.get("/boom-default-msg")
        def _boom_default() -> None:
            raise LumenError(LumenErrorCode.UNAUTHENTICATED)

        return TestClient(app, raise_server_exceptions=False)

    def test_returns_correct_status_and_body(self, client: TestClient) -> None:
        resp = client.get("/boom")
        assert resp.status_code == 404
        body = resp.json()
        assert body["error_code"] == "NOT_FOUND"
        assert body["detail"] == "Thing not found"

    def test_status_code_override_in_response(self, client: TestClient) -> None:
        resp = client.get("/boom-override")
        assert resp.status_code == 503
        body = resp.json()
        assert body["error_code"] == "BAD_GATEWAY"
        assert body["detail"] == "upstream 503"

    def test_default_message(self, client: TestClient) -> None:
        resp = client.get("/boom-default-msg")
        assert resp.status_code == 401
        body = resp.json()
        assert body["error_code"] == "UNAUTHENTICATED"
        assert body["detail"] == "UNAUTHENTICATED"
