# Native modules

Rust modules that replace Python hot paths. Each one is optional: when it is not
built, or its flag is off, the Python code runs unchanged.

## Language policy

Rust is the target language for backend code. The web frontend stays on
Next.js and React.

| Area | Language | Note |
| --- | --- | --- |
| CPU-bound work inside the Python process (parsing, chunking, text cleanup) | Rust | Links into CPython through PyO3. No network hop, no new service, no serialization cost. |
| Standalone backend services (proxies, gateways, HTTP services) | Rust | Predictable latency and memory, no GIL. |
| Web frontend | TypeScript | Next.js and React. Not a migration target. |
| Python still to migrate | Python | The FastAPI app, the Celery workers, and the connectors. Moving each one is its own project. |

`model_server` is the one backend component that stays on Python: it runs torch
and the HuggingFace stack, which have no Rust equivalent.

Rust is already in the tree (`desktop/src-tauri`), so the native modules add no
new toolchain.

## lumen_text

Replaces the HTML-to-text and text-cleanup primitives used by every connector
and by the indexing pipeline.

| Python function | Module |
| --- | --- |
| `parse_html_page_basic` | `lumen/file_processing/html_utils.py` |
| `web_html_cleanup` | `lumen/file_processing/html_utils.py` |
| `clean_text` | `lumen/utils/text_processing.py` |
| `shared_precompare_cleanup` | `lumen/utils/text_processing.py` |

### Build

```bash
backend/native/build.sh          # needs a Rust toolchain: https://rustup.rs
export LUMEN_NATIVE_TEXT=true    # opt in
```

The build produces an abi3 module that works on CPython 3.11 and later. Without
`LUMEN_NATIVE_TEXT=true` the module is never imported.

### Tests

```bash
cd backend/native/lumen_text && cargo test --no-default-features
uv run pytest backend/tests/unit/lumen/file_processing/test_native_text_parity.py
```

The Rust tests need `--no-default-features` so the test binary can link against
libpython. The pytest file runs each input through both paths and asserts the
results are identical; it skips when the module is not built.

### Parity

The two paths agree byte-for-byte on well-formed HTML. Measured on this
checkout:

- The fixed-case pytest suite: 164 checks, 0 divergences.
- 3 000 generated well-formed documents: 0 divergences.

They disagree on malformed markup, because lxml (libxml2) and html5ever apply
different recovery rules:

- `<tr>`, `<td>`, `<th>` outside a `<table>`
- block content or another heading inside `<h1>`-`<h6>`
- tags inside `<script>`

On a generator that produces such markup deliberately, 27% of documents differ.
Real pages are mostly well-formed, but crawled HTML is not guaranteed to be,
which is why the flag is off by default. Validate against your own crawl before
turning it on.

To reproduce these numbers, or to check the native path against your own crawl
corpus before turning it on, see `parity/README.md`.

Three lxml behaviours the Rust code reproduces on purpose, because the Python
output depends on them:

1. A text node made only of ASCII whitespace collapses to one character: `"\n"`
   when it holds a line break, `" "` otherwise. Not applied under `<pre>` or
   `<textarea>`.
2. Text inside `<script>`, `<style>`, `<template>`, `<rt>` and `<rp>` reports as
   empty through BeautifulSoup, so it never reaches the output.
3. `\r` normalizes to `\n` before rule 1 applies.

### Measured speedup

`parse_html_page_basic` and `web_html_cleanup`, against bs4 + lxml:

| Input | parse_html_page_basic | web_html_cleanup |
| --- | --- | --- |
| 1 KB page | 33x | 52x |
| 10 KB page | 37x | 50x |
| 58 KB page | 37x | 54x |

`clean_text` is 23x faster and `shared_precompare_cleanup` 12x on a 37 KB input.

Reproduce with `parity/bench.py`.

Both HTML functions release the GIL while they work, so threaded Celery workers
gain concurrency as well as speed.
