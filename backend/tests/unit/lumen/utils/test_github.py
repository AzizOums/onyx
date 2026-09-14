"""Unit tests for lumen.utils.github."""

from collections.abc import Iterable
from unittest.mock import MagicMock, patch

import pytest
import requests

from lumen.error_handling.error_codes import LumenErrorCode
from lumen.error_handling.exceptions import LumenError
from lumen.utils.github import (
    GitHubSource,
    download_github_archive,
    parse_github_source,
)

_SOURCE = GitHubSource(owner="lumen-dot-app", repo="lumen")


class TestParseGithubSource:
    def test_preserves_unsplit_tree_tail_for_revision_resolution(self) -> None:
        source = parse_github_source(
            "https://github.com/lumen-dot-app/lumen/tree/feature/foo/backend/lumen",
            allow_tree_path=True,
        )

        assert source.owner == "lumen-dot-app"
        assert source.repo == "lumen"
        assert source.tree_tail == ("feature", "foo", "backend", "lumen")

    def test_coding_agent_can_accept_ssh_clone_url(self) -> None:
        parsed = parse_github_source(
            "git@github.com:lumen-dot-app/lumen.git",
            allow_ssh=True,
        )

        assert parsed.owner == "lumen-dot-app"
        assert parsed.repo == "lumen"

    @pytest.mark.parametrize(
        "source",
        [
            "https://github.com/lumen-dot-app/lumen/tree/main",
            "https://github.com/lumen-dot-app/lumen/issues/123",
            "https://github.com/lumen-dot-app/lumen/pull/123",
            "https://github.com/lumen-dot-app/lumen/blob/main/README.md",
        ],
    )
    def test_coding_agent_rejects_repository_subpages(self, source: str) -> None:
        with pytest.raises(LumenError) as exc_info:
            parse_github_source(
                source,
                allow_ssh=True,
            )

        assert exc_info.value.error_code == LumenErrorCode.INVALID_INPUT
        assert "without a branch, file, or subpage" in exc_info.value.detail

    def test_coding_agent_rejects_http_repository_url(self) -> None:
        with pytest.raises(LumenError) as exc_info:
            parse_github_source(
                "http://github.com/lumen-dot-app/lumen",
                allow_ssh=True,
            )

        assert exc_info.value.error_code == LumenErrorCode.INVALID_INPUT

    @pytest.mark.parametrize(
        "source",
        [
            "http://github.com/lumen-dot-app/lumen",
            "git@github.com:lumen-dot-app/lumen",
            "https://gitlab.com/lumen-dot-app/lumen",
            "https://github.com/lumen-dot-app/lumen/issues",
        ],
    )
    def test_rejects_sources_outside_the_import_contract(self, source: str) -> None:
        with pytest.raises(LumenError) as exc_info:
            parse_github_source(source)

        assert exc_info.value.error_code == LumenErrorCode.INVALID_INPUT


def _mock_response(
    chunks: Iterable[bytes],
    status_code: int = 200,
    headers: dict[str, str] | None = None,
) -> MagicMock:
    """Build a MagicMock that quacks like a streamed ``requests.Response``."""
    response = MagicMock(spec=requests.Response)
    response.status_code = status_code
    response.headers = headers or {}
    response.iter_content.return_value = iter(list(chunks))
    if status_code >= 400:
        response.raise_for_status.side_effect = requests.HTTPError(
            f"HTTP {status_code}"
        )
    else:
        response.raise_for_status.return_value = None
    response.__enter__.return_value = response
    response.__exit__.return_value = False
    return response


class TestDownloadGithubArchive:
    def test_returns_concatenated_body(self) -> None:
        response = _mock_response([b"foo", b"bar", b"baz"])
        with patch(
            "lumen.utils.github.ssrf_safe_get", return_value=response
        ) as mock_get:
            result = download_github_archive(
                _SOURCE,
                "HEAD",
                max_size_bytes=500 * 1024 * 1024,
            )

        assert result == b"foobarbaz"
        mock_get.assert_called_once()

    def test_skips_empty_keepalive_chunks(self) -> None:
        response = _mock_response([b"foo", b"", b"bar"])
        with patch("lumen.utils.github.ssrf_safe_get", return_value=response):
            assert (
                download_github_archive(
                    _SOURCE,
                    "HEAD",
                    max_size_bytes=500 * 1024 * 1024,
                )
                == b"foobar"
            )

    def test_builds_tarball_url_from_owner_and_name(self) -> None:
        response = _mock_response([b""])
        with patch(
            "lumen.utils.github.ssrf_safe_get", return_value=response
        ) as mock_get:
            download_github_archive(
                parse_github_source("https://github.com/lumen-dot-app/lumen.git"),
                "HEAD",
                max_size_bytes=500 * 1024 * 1024,
            )

        called_url = mock_get.call_args.args[0]
        assert called_url == "https://codeload.github.com/lumen-dot-app/lumen/tar.gz/HEAD"

    def test_uses_token_only_for_private_repo_fallback(self) -> None:
        public_response = _mock_response([], status_code=404)
        redirect_response = _mock_response(
            [],
            status_code=302,
            headers={"Location": "https://codeload.github.com/signed/archive"},
        )
        archive_response = _mock_response([b"archive"])
        with patch(
            "lumen.utils.github.ssrf_safe_get",
            side_effect=[public_response, redirect_response, archive_response],
        ) as mock_get:
            download_github_archive(
                _SOURCE,
                "HEAD",
                "Bearer ghp_secret",
                max_size_bytes=500 * 1024 * 1024,
            )

        public_request, authenticated_request, archive_request = (
            call.kwargs for call in mock_get.call_args_list
        )
        assert public_request["headers"] is None
        assert authenticated_request["headers"]["Authorization"] == "Bearer ghp_secret"
        assert authenticated_request["follow_redirects"] is False
        assert archive_request["headers"] is None

    def test_omits_authorization_header_when_no_token(self) -> None:
        response = _mock_response([b""])
        with patch(
            "lumen.utils.github.ssrf_safe_get", return_value=response
        ) as mock_get:
            download_github_archive(
                _SOURCE,
                "HEAD",
                max_size_bytes=500 * 1024 * 1024,
            )

        assert mock_get.call_args.kwargs["headers"] is None

    def test_uses_streaming_with_split_timeout(self) -> None:
        response = _mock_response([b""])
        with patch(
            "lumen.utils.github.ssrf_safe_get", return_value=response
        ) as mock_get:
            download_github_archive(
                _SOURCE,
                "HEAD",
                max_size_bytes=500 * 1024 * 1024,
                timeout=(30, 300),
            )

        kwargs = mock_get.call_args.kwargs
        assert kwargs["stream"] is True
        assert kwargs["follow_redirects"] is True
        assert kwargs["timeout"] == (30, 300)

    def test_raises_when_exceeds_max_size(self) -> None:
        response = _mock_response([b"x" * 10, b"x" * 10])
        with patch("lumen.utils.github.ssrf_safe_get", return_value=response):
            with pytest.raises(LumenError) as exc_info:
                download_github_archive(
                    _SOURCE,
                    "HEAD",
                    max_size_bytes=15,
                )

        assert exc_info.value.error_code == LumenErrorCode.PAYLOAD_TOO_LARGE

    def test_propagates_http_errors(self) -> None:
        response = _mock_response([], status_code=404)
        with patch("lumen.utils.github.ssrf_safe_get", return_value=response):
            with pytest.raises(LumenError) as exc_info:
                download_github_archive(
                    _SOURCE,
                    "HEAD",
                    max_size_bytes=500 * 1024 * 1024,
                )

        assert exc_info.value.error_code == LumenErrorCode.NOT_FOUND
