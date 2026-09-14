//! Python bindings for the Lumen text extraction primitives.
//!
//! Every function here is a drop-in replacement for the Python original. The
//! Python side (`lumen/file_processing/native.py`) loads this module when it is
//! built and falls back to the pure-Python code otherwise, so the extension is
//! always optional.

use pyo3::prelude::*;

pub mod html;
pub mod text;

use html::{CleanupOptions, FormatOptions};

#[pyfunction]
#[pyo3(name = "strip_newlines")]
fn py_strip_newlines(document: &str) -> String {
    text::strip_newlines(document)
}

#[pyfunction]
#[pyo3(name = "strip_excessive_newlines_and_spaces")]
fn py_strip_excessive_newlines_and_spaces(document: &str) -> String {
    text::strip_excessive_newlines_and_spaces(document)
}

#[pyfunction]
#[pyo3(name = "shared_precompare_cleanup")]
fn py_shared_precompare_cleanup(text_input: &str) -> String {
    text::shared_precompare_cleanup(text_input)
}

#[pyfunction]
#[pyo3(name = "clean_text")]
fn py_clean_text(text_input: &str) -> String {
    text::clean_text(text_input)
}

#[pyfunction]
#[pyo3(name = "parse_html_page_basic")]
#[pyo3(signature = (html_content, table_cell_separator = "\t", markdown_links = false))]
fn py_parse_html_page_basic(
    py: Python<'_>,
    html_content: &str,
    table_cell_separator: &str,
    markdown_links: bool,
) -> String {
    let opts = FormatOptions {
        table_cell_separator: table_cell_separator.to_string(),
        markdown_links,
    };
    py.detach(|| html::parse_html_page_basic(html_content, &opts))
}

#[pyfunction]
#[pyo3(name = "web_html_cleanup")]
#[pyo3(signature = (
    html_content,
    unwanted_classes,
    unwanted_tags,
    table_cell_separator = "\t",
    markdown_links = false,
))]
fn py_web_html_cleanup(
    py: Python<'_>,
    html_content: &str,
    unwanted_classes: Vec<String>,
    unwanted_tags: Vec<String>,
    table_cell_separator: &str,
    markdown_links: bool,
) -> (Option<String>, String) {
    let opts = CleanupOptions {
        unwanted_classes,
        unwanted_tags,
        format: FormatOptions {
            table_cell_separator: table_cell_separator.to_string(),
            markdown_links,
        },
    };
    py.detach(|| html::web_html_cleanup(html_content, &opts))
}

#[pymodule]
fn lumen_text_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(py_strip_newlines, m)?)?;
    m.add_function(wrap_pyfunction!(py_strip_excessive_newlines_and_spaces, m)?)?;
    m.add_function(wrap_pyfunction!(py_shared_precompare_cleanup, m)?)?;
    m.add_function(wrap_pyfunction!(py_clean_text, m)?)?;
    m.add_function(wrap_pyfunction!(py_parse_html_page_basic, m)?)?;
    m.add_function(wrap_pyfunction!(py_web_html_cleanup, m)?)?;
    Ok(())
}
