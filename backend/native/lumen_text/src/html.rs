//! HTML to flat-text extraction.
//!
//! Mirrors `format_document_soup` and `web_html_cleanup` in
//! `lumen/file_processing/html_utils.py`. The Python versions walk
//! `BeautifulSoup.descendants` in document order and look up ancestors per node;
//! here the same state travels down an explicit pre-order stack, which gives the
//! same result without repeated parent walks.

use ego_tree::NodeId;
use scraper::{Html, Node};
use std::borrow::Cow;
use std::rc::Rc;

use crate::text::{is_py_space, py_strip, strip_excessive_newlines_and_spaces, strip_newlines};

const ZERO_WIDTH_SPACE: char = '\u{200b}';

/// Tags whose text children BeautifulSoup wraps in a dedicated `NavigableString`
/// subclass (`Script`, `Stylesheet`, `TemplateString`, `RubyTextString`,
/// `RubyParenthesisString`). Those subclasses report an empty `.text`, so the
/// Python walker adds nothing for them and their content never reaches the index.
const EMPTY_TEXT_PARENTS: [&str; 5] = ["script", "style", "template", "rt", "rp"];

/// Tags under which libxml2 keeps whitespace verbatim.
const WHITESPACE_PRESERVING: [&str; 2] = ["pre", "textarea"];

/// ASCII whitespace as libxml2 sees it. U+00A0 is deliberately excluded.
#[inline]
fn is_html_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{0c}')
}

/// Collapse a blank text node the way libxml2 does.
///
/// A text node made only of ASCII whitespace becomes a single character: `"\n"`
/// when the run holds any line break (libxml2 normalizes `\r` to `\n` first),
/// `" "` otherwise. Nodes with real content, and anything under `<pre>` or
/// `<textarea>`, pass through untouched. The Python path gets this from lxml, so
/// the native path has to reproduce it to stay byte-identical.
///
/// Verified against bs4 + lxml over every whitespace run up to length four.
fn normalize_blank_text(raw: &str, in_preserve: bool) -> Cow<'_, str> {
    if in_preserve || raw.is_empty() || !raw.chars().all(is_html_space) {
        return Cow::Borrowed(raw);
    }
    if raw.contains('\n') || raw.contains('\r') {
        Cow::Borrowed("\n")
    } else {
        Cow::Borrowed(" ")
    }
}

pub struct FormatOptions {
    pub table_cell_separator: String,
    /// True for the MARKDOWN link strategy, false for STRIP.
    pub markdown_links: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            table_cell_separator: "\t".to_string(),
            markdown_links: false,
        }
    }
}

pub struct CleanupOptions {
    pub unwanted_classes: Vec<String>,
    pub unwanted_tags: Vec<String>,
    pub format: FormatOptions,
}

/// One pending node in the pre-order walk, carrying the ancestor state that the
/// Python code recomputes with `find_parent`.
struct Frame {
    id: NodeId,
    in_table: bool,
    in_preserve: bool,
    link: Option<Rc<str>>,
}

/// Python `str.split()` on the class attribute: split on runs of whitespace.
fn class_tokens(attr: &str) -> impl Iterator<Item = &str> {
    attr.split(is_py_space).filter(|tok| !tok.is_empty())
}

/// Mirrors `format_element_text`. An empty href is falsy in Python, so it also
/// yields the plain text.
fn format_element_text(
    element_text: &str,
    link_href: Option<&str>,
    markdown_links: bool,
) -> String {
    let no_newlines = strip_newlines(element_text);
    match link_href {
        Some(href) if markdown_links && !href.is_empty() => {
            format!("[{no_newlines}]({href})")
        }
        _ => no_newlines,
    }
}

fn push_children(
    stack: &mut Vec<Frame>,
    doc: &Html,
    frame: &Frame,
    name: &str,
    href: Option<&str>,
) {
    let in_table = frame.in_table || name == "table";
    let in_preserve = frame.in_preserve || WHITESPACE_PRESERVING.contains(&name);
    // An `<a>` shadows any outer anchor, even when it carries no href.
    let link: Option<Rc<str>> = if name == "a" {
        href.map(Rc::from)
    } else {
        frame.link.clone()
    };

    let node = doc.tree.get(frame.id).expect("frame id is in the tree");
    // Reverse so the stack pops children in document order.
    for child in node.children().collect::<Vec<_>>().into_iter().rev() {
        stack.push(Frame {
            id: child.id(),
            in_table,
            in_preserve,
            link: link.clone(),
        });
    }
}

/// Flatten a parsed document to text. Mirrors `format_document_soup`.
// The heading and <br> arms are identical on purpose: they are separate branches in
// the Python source and keeping them apart makes the two easy to diff.
#[allow(clippy::if_same_then_else)]
pub fn format_document_soup(doc: &Html, opts: &FormatOptions) -> String {
    let mut text = String::new();
    let mut list_element_start = false;
    let mut verbatim_output: i64 = 0;
    let mut last_added_newline = false;

    let root = doc.tree.root();
    let mut stack: Vec<Frame> = root
        .children()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|child| Frame {
            id: child.id(),
            in_table: false,
            in_preserve: false,
            link: None,
        })
        .collect();

    while let Some(frame) = stack.pop() {
        verbatim_output -= 1;
        let node = doc.tree.get(frame.id).expect("frame id is in the tree");

        match node.value() {
            Node::Text(raw) => {
                let in_empty_text_parent = node
                    .parent()
                    .and_then(|parent| parent.value().as_element().map(|el| el.name().to_string()))
                    .is_some_and(|name| EMPTY_TEXT_PARENTS.contains(&name.as_str()));
                if in_empty_text_parent {
                    continue;
                }

                let mut element_text = normalize_blank_text(raw, frame.in_preserve);

                if frame.in_table {
                    // Table rows are newline separated, so cells must not hold newlines.
                    let replaced = element_text.replace('\n', " ");
                    element_text = Cow::Owned(py_strip(&replaced).to_string());
                }

                if last_added_newline && element_text.starts_with(' ') {
                    element_text = Cow::Owned(element_text[1..].to_string());
                    last_added_newline = false;
                }

                if element_text.is_empty() {
                    continue;
                }

                let content_to_add = if verbatim_output > 0 {
                    element_text.into_owned()
                } else {
                    let href = if frame.in_table {
                        None
                    } else {
                        frame.link.as_deref()
                    };
                    format_element_text(&element_text, href, opts.markdown_links)
                };

                // Don't join separate elements without any spacing.
                let joins_words = text.chars().next_back().is_some_and(|c| !is_py_space(c))
                    && content_to_add
                        .chars()
                        .next()
                        .is_some_and(|c| !is_py_space(c));
                if joins_words {
                    text.push(' ');
                }

                text.push_str(&content_to_add);
                list_element_start = false;
            }

            Node::Element(el) => {
                let name = el.name();
                let in_table = frame.in_table;

                if name == "tr" && in_table {
                    text.push('\n');
                } else if (name == "td" || name == "th") && in_table {
                    text.push_str(&opts.table_cell_separator);
                } else if in_table {
                    // Other tags are ignored while inside a table.
                } else if name == "p" || name == "div" {
                    if !list_element_start {
                        text.push('\n');
                    }
                } else if matches!(name, "h1" | "h2" | "h3" | "h4") {
                    text.push('\n');
                    list_element_start = false;
                    last_added_newline = true;
                } else if name == "br" {
                    text.push('\n');
                    list_element_start = false;
                    last_added_newline = true;
                } else if name == "li" {
                    text.push_str("\n- ");
                    list_element_start = true;
                } else if name == "pre" && verbatim_output <= 0 {
                    verbatim_output = node.children().count() as i64;
                }

                let href = el.attr("href");
                // `name` and `href` borrow the node, so pass them along explicitly.
                push_children(&mut stack, doc, &frame, name, href);
            }

            // Comments and doctypes are skipped; they have no children.
            _ => {}
        }
    }

    strip_excessive_newlines_and_spaces(&text)
}

/// Parse then flatten. Mirrors `parse_html_page_basic`.
pub fn parse_html_page_basic(html: &str, opts: &FormatOptions) -> String {
    format_document_soup(&Html::parse_document(html), opts)
}

/// First element with this tag name in document order.
fn find_first_element(doc: &Html, tag: &str) -> Option<NodeId> {
    let mut stack: Vec<NodeId> = doc
        .tree
        .root()
        .children()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|c| c.id())
        .collect();

    while let Some(id) = stack.pop() {
        let node = doc.tree.get(id).expect("stack id is in the tree");
        if let Node::Element(el) = node.value() {
            if el.name() == tag {
                return Some(id);
            }
        }
        for child in node.children().collect::<Vec<_>>().into_iter().rev() {
            stack.push(child.id());
        }
    }
    None
}

/// Concatenated text of every descendant. Mirrors BeautifulSoup's `Tag.text`.
fn element_text(doc: &Html, id: NodeId) -> String {
    let mut out = String::new();
    let node = doc.tree.get(id).expect("id is in the tree");
    let mut in_preserve = node.ancestors().chain(std::iter::once(node)).any(|a| {
        a.value()
            .as_element()
            .is_some_and(|el| WHITESPACE_PRESERVING.contains(&el.name()))
    });

    for descendant in node.descendants() {
        if let Node::Element(el) = descendant.value() {
            in_preserve = in_preserve || WHITESPACE_PRESERVING.contains(&el.name());
        }
        if let Node::Text(raw) = descendant.value() {
            out.push_str(&normalize_blank_text(raw, in_preserve));
        }
    }
    out
}

/// Strip boilerplate then flatten to text. Mirrors the BeautifulSoup branch of
/// `web_html_cleanup`; the trafilatura branch stays in Python.
pub fn web_html_cleanup(html: &str, opts: &CleanupOptions) -> (Option<String>, String) {
    let mut doc = Html::parse_document(html);

    let mut title: Option<String> = None;
    if let Some(title_id) = find_first_element(&doc, "title") {
        let text = element_text(&doc, title_id);
        if !text.is_empty() {
            title = Some(text);
            doc.tree
                .get_mut(title_id)
                .expect("title id is in the tree")
                .detach();
        }
    }

    let mut to_detach: Vec<NodeId> = Vec::new();
    for node in doc.tree.nodes() {
        let Node::Element(el) = node.value() else {
            continue;
        };
        let unwanted_tag = opts.unwanted_tags.iter().any(|tag| tag == el.name());
        let unwanted_class = el.attr("class").is_some_and(|attr| {
            class_tokens(attr).any(|tok| opts.unwanted_classes.iter().any(|u| u == tok))
        });
        if unwanted_tag || unwanted_class {
            to_detach.push(node.id());
        }
    }
    for id in to_detach {
        // Already-orphaned nodes detach harmlessly.
        doc.tree.get_mut(id).expect("id is in the tree").detach();
    }

    let page_text = format_document_soup(&doc, &opts.format);
    let cleaned_text = page_text.replace(ZERO_WIDTH_SPACE, "");
    (title, cleaned_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basic(html: &str) -> String {
        parse_html_page_basic(html, &FormatOptions::default())
    }

    #[test]
    fn table_cells_are_tab_separated_and_rows_newline_separated() {
        let html = "<table><tr><td>hello</td><td>there</td></tr><tr><td>kenobi</td></tr></table>";
        // The leading "\n\t" the first row emits is removed by the final strip.
        assert_eq!(basic(html), "hello\tthere\n\tkenobi");
    }

    #[test]
    fn script_and_style_content_is_dropped() {
        assert_eq!(basic("<script>var x = 1;</script><p>after</p>"), "after");
        assert_eq!(basic("<style>.a{color:red}</style><p>after</p>"), "after");
    }

    #[test]
    fn whitespace_only_title_collapses_like_libxml2() {
        let opts = CleanupOptions {
            unwanted_classes: vec![],
            unwanted_tags: vec![],
            format: FormatOptions::default(),
        };
        let (title, _) = web_html_cleanup("<html><head><title>\t\t</title></head></html>", &opts);
        assert_eq!(title.as_deref(), Some(" "));
        let (title, _) = web_html_cleanup("<html><head><title>\n  </title></head></html>", &opts);
        assert_eq!(title.as_deref(), Some("\n"));
        let (title, _) = web_html_cleanup("<html><head><title> Real </title></head></html>", &opts);
        assert_eq!(title.as_deref(), Some(" Real "));
    }

    #[test]
    fn block_formatting_resumes_after_a_table() {
        let html = "<p>before</p>\
                    <table><tr><td>cell</td></tr></table>\
                    <h2>after heading</h2>\
                    <p>after paragraph</p>\
                    <ul><li>one</li><li>two</li></ul>";
        assert_eq!(
            basic(html),
            "before\n\tcell\nafter heading\nafter paragraph\n- one\n- two"
        );
    }

    #[test]
    fn markdown_links_stop_at_the_anchor() {
        let html = "<p>See <a href=\"https://example.com\">this link</a> now.</p>\
                    <p>Next paragraph.</p>";
        let opts = FormatOptions {
            table_cell_separator: "\t".to_string(),
            markdown_links: true,
        };
        assert_eq!(
            parse_html_page_basic(html, &opts),
            "See [this link](https://example.com) now.\nNext paragraph."
        );
    }

    #[test]
    fn links_are_stripped_by_default() {
        let html = "<p>See <a href=\"https://example.com\">this link</a> now.</p>";
        assert_eq!(basic(html), "See this link now.");
    }

    #[test]
    fn cleanup_extracts_title_and_drops_boilerplate() {
        let html = "<html><head><title>My Page</title></head><body>\
                    <nav>menu</nav><div class=\"sidebar\">junk</div>\
                    <p>real content</p></body></html>";
        let opts = CleanupOptions {
            unwanted_classes: vec!["sidebar".to_string()],
            unwanted_tags: vec!["nav".to_string()],
            format: FormatOptions::default(),
        };
        let (title, text) = web_html_cleanup(html, &opts);
        assert_eq!(title.as_deref(), Some("My Page"));
        assert_eq!(text, "real content");
    }

    #[test]
    fn class_match_is_on_whole_tokens() {
        let html = "<p class=\"sidebar-wide\">kept</p><p class=\"a sidebar\">dropped</p>";
        let opts = CleanupOptions {
            unwanted_classes: vec!["sidebar".to_string()],
            unwanted_tags: vec![],
            format: FormatOptions::default(),
        };
        let (_, text) = web_html_cleanup(html, &opts);
        assert_eq!(text, "kept");
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(basic("<p>a<!-- note -->b</p>"), "a b");
    }
}
