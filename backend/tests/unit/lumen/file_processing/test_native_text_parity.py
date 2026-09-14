"""Parity between the Rust text module and the Python implementation.

Skipped when the native module is not built. Build it with
`backend/native/build.sh`.

Scope: the two paths agree byte-for-byte on well-formed HTML. Malformed markup
(stray `<tr>`/`<td>` outside a table, block content inside a heading, tags inside
`<script>`) is out of scope, because lxml and html5ever apply different recovery
rules there. `backend/native/README.md` records the measured divergence.
"""

from collections.abc import Callable
from typing import Any

import pytest

import lumen.file_processing.html_utils as html_utils
import lumen.utils.text_processing as text_processing
from lumen.file_processing.enums import HtmlBasedConnectorTransformLinksStrategy

native = pytest.importorskip(
    "lumen_text_native",
    reason="native text module not built; run backend/native/build.sh",
)


HTML_CASES = [
    "",
    "<p>hello</p>",
    "<p>before</p><table><tr><td>cell</td></tr></table><h2>after heading</h2>"
    "<p>after paragraph</p><ul><li>one</li><li>two</li></ul>",
    "<table><tr><td>hello</td><td>there</td><td>general</td></tr>"
    "<tr><td>kenobi</td><td>a</td><td>b</td></tr></table>",
    '<p>See <a href="https://example.com">this link</a> now.</p><p>Next paragraph.</p>',
    '<p><a>no href</a><a href="">empty href</a></p>',
    '<table><tr><td><a href="h">link in table</a></td></tr></table>',
    "<pre>  verbatim\n   spacing  </pre>",
    "<pre><code>a</code> <code>b</code>\n<code>c</code></pre>",
    "<div><p>a</p><p>b</p></div>",
    "<h1>h1</h1><h2>h2</h2><h3>h3</h3><h4>h4</h4><h5>h5</h5>",
    "<p>line<br>break<br/>again</p>",
    "<ul><li>a</li><li>b</li></ul><div>after list</div>",
    "<p>a<!-- comment -->b</p>",
    "<!DOCTYPE html><html><body><p>doctype</p></body></html>",
    "<p>   leading and trailing   </p>",
    "<p>multi\n\nline\ntext</p>",
    "<table><tr><th>H</th></tr><tr><td>a\nb</td></tr></table>",
    "<table><tbody><tr><td>tbody</td></tr></tbody></table>",
    "<p>&amp; &lt; &gt; &nbsp; &#8203;</p>",
    "<p>unicode — dash ​ zwsp \U0001f600 emoji</p>",
    "<script>var x = 1;</script><p>after script</p>",
    "<style>.a{color:red}</style><p>after style</p>",
    "<html><head><title>My Title</title></head><body><p>body</p></body></html>",
    "<html><head><title></title></head><body><p>empty title</p></body></html>",
    '<div class="sidebar">drop</div><p>keep</p>',
    '<div class="sidebar-wide">keep</div><p>keep2</p>',
    '<div class="a sidebar b">drop</div><p>keep</p>',
    '<div class="sticky">mintlify drop</div><p>keep</p>',
    "<nav>nav</nav><footer>footer</footer><aside>aside</aside><p>keep</p>",
    '<div class="sidebar"><p>nested inside dropped</p></div>',
    "<p>a</p>\n<p>b</p>",
    "<p>" + "x" * 5000 + "</p>",
    "<p>a</p>" * 200,
]

TEXT_CASES = [
    "",
    " ",
    "   a   b  \n\n\n c \r\n d  ",
    "Hello, World. *A*",
    'a\\"b',
    "a\\b",
    "tabs\tand\nnewlines\r\n",
    "— em dash → arrow ✈ dingbat \U0001f600 emoji ￹ special",
    "control\x00chars\x01here\x1f",
    "a\x0bb\x0cc\rd",
    "MiXeD CaSe TeXt",
    " nbsp ",
    "x" * 10000,
]


def _both_paths(
    monkeypatch: pytest.MonkeyPatch,
    module: Any,
    call: Callable[[], Any],
) -> tuple[Any, Any]:
    """Run `call` with the native module off, then on."""
    monkeypatch.setattr(module, "get_native", lambda: None)
    python_result = call()
    monkeypatch.setattr(module, "get_native", lambda: native)
    native_result = call()
    return python_result, native_result


@pytest.mark.parametrize("html", HTML_CASES)
@pytest.mark.parametrize(
    "strategy",
    [
        HtmlBasedConnectorTransformLinksStrategy.STRIP,
        HtmlBasedConnectorTransformLinksStrategy.MARKDOWN,
    ],
)
def test_parse_html_page_basic_matches(
    monkeypatch: pytest.MonkeyPatch, html: str, strategy: str
) -> None:
    monkeypatch.setattr(
        html_utils, "HTML_BASED_CONNECTOR_TRANSFORM_LINKS_STRATEGY", strategy
    )
    python_result, native_result = _both_paths(
        monkeypatch, html_utils, lambda: html_utils.parse_html_page_basic(html)
    )
    assert python_result == native_result


@pytest.mark.parametrize("html", HTML_CASES)
@pytest.mark.parametrize("mintlify", [True, False])
def test_web_html_cleanup_matches(
    monkeypatch: pytest.MonkeyPatch, html: str, mintlify: bool
) -> None:
    python_result, native_result = _both_paths(
        monkeypatch,
        html_utils,
        lambda: html_utils.web_html_cleanup(html, mintlify_cleanup_enabled=mintlify),
    )
    assert python_result.title == native_result.title
    assert python_result.cleaned_text == native_result.cleaned_text


def test_web_html_cleanup_honours_additional_discards(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    html = "<div><span>drop me</span><p>keep me</p></div>"
    python_result, native_result = _both_paths(
        monkeypatch,
        html_utils,
        lambda: html_utils.web_html_cleanup(
            html, additional_element_types_to_discard=["span"]
        ),
    )
    assert python_result.cleaned_text == native_result.cleaned_text
    assert "drop me" not in native_result.cleaned_text


@pytest.mark.parametrize("text", TEXT_CASES)
def test_clean_text_matches(monkeypatch: pytest.MonkeyPatch, text: str) -> None:
    python_result, native_result = _both_paths(
        monkeypatch, text_processing, lambda: text_processing.clean_text(text)
    )
    assert python_result == native_result


@pytest.mark.parametrize("text", TEXT_CASES)
def test_shared_precompare_cleanup_matches(
    monkeypatch: pytest.MonkeyPatch, text: str
) -> None:
    python_result, native_result = _both_paths(
        monkeypatch,
        text_processing,
        lambda: text_processing.shared_precompare_cleanup(text),
    )
    assert python_result == native_result


def test_byte_streams_keep_the_python_path(monkeypatch: pytest.MonkeyPatch) -> None:
    """bs4 handles encoding detection, so byte input must not reach the native path."""
    from io import BytesIO

    monkeypatch.setattr(html_utils, "get_native", lambda: native)
    assert html_utils.parse_html_page_basic(BytesIO(b"<p>bytes</p>")) == "bytes"
