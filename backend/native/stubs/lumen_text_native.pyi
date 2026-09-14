"""Type stub for the optional Rust text module.

The module is built by `backend/native/build.sh` and may be absent, so this stub
lets `ty` check the call sites either way. Keep it in step with
`backend/native/lumen_text/src/lib.rs`.
"""

__version__: str

def strip_newlines(document: str) -> str: ...
def strip_excessive_newlines_and_spaces(document: str) -> str: ...
def shared_precompare_cleanup(text_input: str) -> str: ...
def clean_text(text_input: str) -> str: ...
def parse_html_page_basic(
    html_content: str,
    table_cell_separator: str = "\t",
    markdown_links: bool = False,
) -> str: ...
def web_html_cleanup(
    html_content: str,
    unwanted_classes: list[str],
    unwanted_tags: list[str],
    table_cell_separator: str = "\t",
    markdown_links: bool = False,
) -> tuple[str | None, str]: ...
