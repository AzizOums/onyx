"""Compare the native text path against the Python path.

Both paths come from the real `lumen` modules; the script only toggles which
backend they use, so there is no second copy of the logic to keep in sync.

Run from the repo root:

    uv run python backend/native/parity/check_corpus.py wellformed --count 5000
    uv run python backend/native/parity/check_corpus.py adversarial --count 3000
    uv run python backend/native/parity/check_corpus.py files --path /some/crawl/dir

`files` is the one that matters before turning the flag on: point it at real
pages from your own crawl and confirm it reports zero divergences.
"""

import os
import sys

# Same idiom as backend/scripts/*.py: put `backend/` on the path.
sys.path.append(
    os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
)

import argparse
import random
from collections.abc import Iterator
from pathlib import Path

import lumen.file_processing.html_utils as html_utils
import lumen.utils.native_text as native_text

try:
    import lumen_text_native
except ImportError:
    sys.exit("lumen_text_native is not built. Run backend/native/build.sh")


INLINE = ["b", "i", "span", "code", "em", "strong"]
BLOCK = ["p", "div", "section", "article", "h1", "h2", "h3", "h4"]
NESTABLE = ("div", "section", "article")
WORDS = ["alpha", "beta", " gamma ", "delta\n", "  ", "été", "x", "a b", "—"]
CLASSES = ["sidebar", "footer", "sticky", "hidden", "content", "main", "sidebar-wide"]
ANY_TAG = (
    INLINE
    + BLOCK
    + ["ul", "ol", "li", "table", "tr", "td", "th", "pre", "nav", "script"]
)


def _use_python() -> None:
    native_text.set_native(None)


def _use_native() -> None:
    native_text.set_native(lumen_text_native)


def _maybe_class(rng: random.Random) -> str:
    return ' class="%s"' % rng.choice(CLASSES) if rng.random() < 0.25 else ""


def _text_run(rng: random.Random) -> str:
    return "".join(rng.choice(WORDS) for _ in range(rng.randrange(1, 3)))


def _inline(rng: random.Random, depth: int = 0) -> str:
    parts = []
    for _ in range(rng.randrange(1, 4)):
        roll = rng.random()
        if roll < 0.45 or depth > 2:
            parts.append(_text_run(rng))
        elif roll < 0.6:
            parts.append(
                '<a href="https://e.com/%d">%s</a>'
                % (rng.randrange(100), _text_run(rng))
            )
        elif roll < 0.7:
            parts.append("<br>")
        else:
            tag = rng.choice(INLINE)
            parts.append(
                "<%s%s>%s</%s>" % (tag, _maybe_class(rng), _inline(rng, depth + 1), tag)
            )
    return "".join(parts)


def _block(rng: random.Random, depth: int = 0) -> str:
    roll = rng.random()
    if roll < 0.12:
        rows = []
        for _ in range(rng.randrange(1, 4)):
            cell = "th" if rng.random() < 0.2 else "td"
            cells = "".join(
                "<%s>%s</%s>" % (cell, _inline(rng), cell)
                for _ in range(rng.randrange(1, 4))
            )
            rows.append("<tr>%s</tr>" % cells)
        return "<table%s>%s</table>" % (_maybe_class(rng), "".join(rows))
    if roll < 0.24:
        tag = rng.choice(["ul", "ol"])
        items = "".join(
            "<li>%s</li>" % _inline(rng) for _ in range(rng.randrange(1, 4))
        )
        return "<%s%s>%s</%s>" % (tag, _maybe_class(rng), items, tag)
    if roll < 0.3:
        return "<pre>%s</pre>" % _text_run(rng)
    if roll < 0.36:
        return "<script>var x = %d;</script>" % rng.randrange(100)
    if roll < 0.4:
        return "<style>.a{color:red}</style>"
    if roll < 0.46:
        return "<nav>%s</nav>" % _inline(rng)

    tag = rng.choice(BLOCK)
    # Headings and <p> take phrasing content only; nesting blocks in them is
    # invalid HTML and the two parsers recover differently.
    if tag in NESTABLE and depth < 2 and rng.random() < 0.3:
        inner = "".join(_block(rng, depth + 1) for _ in range(rng.randrange(1, 3)))
    else:
        inner = _inline(rng)
    return "<%s%s>%s</%s>" % (tag, _maybe_class(rng), inner, tag)


def wellformed_docs(rng: random.Random, count: int) -> Iterator[str]:
    for _ in range(count):
        body = "".join(_block(rng) for _ in range(rng.randrange(1, 5)))
        title = rng.choice(["Page Title", "", "  ", "Doc — %d" % rng.randrange(99)])
        yield (
            "<!DOCTYPE html><html><head><title>%s</title></head><body>%s</body></html>"
            % (title, body)
        )


def _adversarial_node(rng: random.Random, depth: int = 0) -> str:
    if depth > 4 or rng.random() < 0.3:
        return rng.choice(WORDS)
    tag = rng.choice(ANY_TAG)
    attrs = ""
    if tag == "a" and rng.random() < 0.8:
        attrs = ' href="https://e.com/%d"' % rng.randrange(100)
    elif rng.random() < 0.3:
        attrs = ' class="%s"' % rng.choice(CLASSES)
    inner = "".join(
        _adversarial_node(rng, depth + 1) for _ in range(rng.randrange(0, 4))
    )
    return "<%s%s>%s</%s>" % (tag, attrs, inner, tag)


def adversarial_docs(rng: random.Random, count: int) -> Iterator[str]:
    """Deliberately invalid nesting, to show where the two parsers part ways."""
    for _ in range(count):
        yield "".join(_adversarial_node(rng) for _ in range(rng.randrange(1, 5)))


def file_docs(path: Path) -> Iterator[str]:
    for html_file in sorted(path.rglob("*.htm*")):
        yield html_file.read_text(encoding="utf-8", errors="replace")


def compare(docs: Iterator[str], show: int) -> int:
    checked = 0
    diverged = 0
    shown = 0

    for doc in docs:
        checked += 1

        _use_python()
        py_basic = html_utils.parse_html_page_basic(doc)
        py_clean = html_utils.web_html_cleanup(doc)

        _use_native()
        rs_basic = html_utils.parse_html_page_basic(doc)
        rs_clean = html_utils.web_html_cleanup(doc)

        same = py_basic == rs_basic and (
            (py_clean.title, py_clean.cleaned_text)
            == (rs_clean.title, rs_clean.cleaned_text)
        )
        if same:
            continue

        diverged += 1
        if shown < show:
            shown += 1
            print("-" * 70)
            print("input:  %r" % doc[:300])
            if py_basic != rs_basic:
                print("  parse_html_page_basic")
                print("    python: %r" % py_basic[:300])
                print("    rust:   %r" % rs_basic[:300])
            if (py_clean.title, py_clean.cleaned_text) != (
                rs_clean.title,
                rs_clean.cleaned_text,
            ):
                print("  web_html_cleanup")
                print(
                    "    python: %r" % ((py_clean.title, py_clean.cleaned_text[:300]),)
                )
                print(
                    "    rust:   %r" % ((rs_clean.title, rs_clean.cleaned_text[:300]),)
                )

    if not checked:
        print("no documents checked")
        return 1

    print()
    print(
        "documents: %d   divergences: %d (%.2f%%)"
        % (checked, diverged, 100.0 * diverged / checked)
    )
    return 1 if diverged else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["wellformed", "adversarial", "files"])
    parser.add_argument("--count", type=int, default=3000, help="generated documents")
    parser.add_argument("--path", type=Path, help="directory of .html files")
    parser.add_argument("--seed", type=int, default=20260914)
    parser.add_argument("--show", type=int, default=3, help="divergences to print")
    args = parser.parse_args()

    rng = random.Random(args.seed)
    if args.mode == "wellformed":
        docs = wellformed_docs(rng, args.count)
    elif args.mode == "adversarial":
        docs = adversarial_docs(rng, args.count)
    else:
        if not args.path:
            parser.error("files mode needs --path")
        docs = file_docs(args.path)

    return compare(docs, args.show)


if __name__ == "__main__":
    sys.exit(main())
