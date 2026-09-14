# Parity harness

Two scripts that compare the native text path against the Python path. Both
import the real `lumen` modules and only toggle which backend they use, so there
is no second copy of the logic to keep in sync.

Build the module first (`backend/native/build.sh`). Neither script needs
`LUMEN_NATIVE_TEXT`; they switch backends themselves.

## Before turning the flag on

Point the harness at real pages from your own crawl and confirm it reports zero
divergences:

```bash
uv run python backend/native/parity/check_corpus.py files --path /some/crawl/dir
```

It reads every `.htm`/`.html` file under the directory, runs both paths, and
prints the divergence count. Exit status is non-zero when anything differs.

## Generated corpora

```bash
# Well-formed HTML. Expected: 0 divergences.
uv run python backend/native/parity/check_corpus.py wellformed --count 5000

# Deliberately invalid nesting. Expected: roughly 15% divergence.
uv run python backend/native/parity/check_corpus.py adversarial --count 3000
```

The adversarial run is there to show the boundary, not to be fixed: lxml and
html5ever recover from broken markup differently, and nothing in the port can
change that. See the parity section of `../README.md`.

## Benchmark

```bash
uv run python backend/native/parity/bench.py
```

## Fixed cases

The fixed-case parity suite runs under pytest and is the one CI should keep
green:

```bash
uv run pytest backend/tests/unit/lumen/file_processing/test_native_text_parity.py
```
