"""Throughput of the native text path against the Python path.

Run from the repo root:

    uv run python backend/native/parity/bench.py
"""

import os
import sys

# Same idiom as backend/scripts/*.py: put `backend/` on the path.
sys.path.append(
    os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
)

import time
from collections.abc import Callable

import lumen.file_processing.html_utils as html_utils
import lumen.utils.native_text as native_text
import lumen.utils.text_processing as text_processing

try:
    import lumen_text_native
except ImportError:
    sys.exit("lumen_text_native is not built. Run backend/native/build.sh")


def _use_python() -> None:
    native_text.set_native(None)


def _use_native() -> None:
    native_text.set_native(lumen_text_native)


def make_page(rows: int, paragraphs: int) -> str:
    body = [
        "<nav><ul><li>Home</li><li>Docs</li><li>API</li></ul></nav>",
        '<div class="sidebar">Navigation junk that gets dropped.</div>',
    ]
    body.extend(
        "<h2>Section %d</h2><p>Some prose with <a href='https://e.com/%d'>a link</a> "
        "and <b>bold</b> plus <code>inline_code()</code> in it. "
        "It runs a little long so the walker has real text to process.</p>" % (i, i)
        for i in range(paragraphs)
    )
    cells = "".join("<td>cell %d</td>" % j for j in range(6))
    body.append(
        "<table>%s</table>" % "".join("<tr>%s</tr>" % cells for _ in range(rows))
    )
    body.append("<script>var tracking = {a: 1, b: 2};</script>")
    body.append("<footer>Footer junk</footer>")
    return (
        "<!DOCTYPE html><html><head><title>Benchmark Page</title></head>"
        "<body>%s</body></html>" % "".join(body)
    )


def timeit(fn: Callable[[], object], iterations: int) -> float:
    start = time.perf_counter()
    for _ in range(iterations):
        fn()
    return time.perf_counter() - start


def compare(label: str, fn: Callable[[], object], iterations: int) -> None:
    _use_python()
    python_seconds = timeit(fn, iterations)
    _use_native()
    native_seconds = timeit(fn, iterations)
    print(
        "  %-22s python %8.1f ms   rust %8.1f ms   speedup %5.1fx"
        % (
            label,
            python_seconds * 1000,
            native_seconds * 1000,
            python_seconds / native_seconds,
        )
    )


def main() -> None:
    print("Python (bs4 + lxml) against Rust\n")
    for label, rows, paragraphs, iterations in (
        ("small page  (1 KB)", 3, 5, 200),
        ("medium page (10 KB)", 30, 40, 100),
        ("large page  (58 KB)", 200, 200, 20),
    ):
        page = make_page(rows, paragraphs)
        print("%s, %d iterations" % (label, iterations))
        compare(
            "parse_html_page_basic",
            lambda p=page: html_utils.parse_html_page_basic(p),
            iterations,
        )
        compare(
            "web_html_cleanup",
            lambda p=page: html_utils.web_html_cleanup(p),
            iterations,
        )

    text = "Some Quoted, Text. " * 2000
    print("\ntext helpers (%d KB), 2000 iterations" % (len(text) // 1024))
    compare("clean_text", lambda: text_processing.clean_text(text), 2000)
    compare(
        "shared_precompare_cleanup",
        lambda: text_processing.shared_precompare_cleanup(text),
        2000,
    )


if __name__ == "__main__":
    main()
