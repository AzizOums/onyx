//! Pure text cleanup primitives.
//!
//! Each function keeps byte-for-byte parity with its Python counterpart in
//! `lumen/utils/text_processing.py` and `lumen/file_processing/html_utils.py`.

use regex::Regex;
use std::sync::LazyLock;

/// Python `str.isspace()`.
///
/// Rust `char::is_whitespace` uses the Unicode White_Space property. Python also
/// treats the four separator controls U+001C-U+001F as space, so add them.
#[inline]
pub fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Python `str.strip()` with no argument.
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(is_py_space)
}

static RE_SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" +").unwrap());
static RE_SPACES_BEFORE_NEWLINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r" +[\n\r]").unwrap());
static RE_NEWLINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\n\r]+").unwrap());

/// Collapse repeated spaces and newlines, then strip. Mirrors
/// `strip_excessive_newlines_and_spaces`.
pub fn strip_excessive_newlines_and_spaces(document: &str) -> String {
    let out = RE_SPACES.replace_all(document, " ");
    let out = RE_SPACES_BEFORE_NEWLINE.replace_all(&out, "\n");
    let out = RE_NEWLINES.replace_all(&out, "\n");
    py_strip(&out).to_string()
}

/// Browsers treat newlines inside HTML as plain spaces. Mirrors `strip_newlines`.
pub fn strip_newlines(document: &str) -> String {
    RE_NEWLINES.replace_all(document, " ").into_owned()
}

/// Lowercase, then drop whitespace, `*`, `\"` and ``.,:`"#-``.
/// Mirrors `shared_precompare_cleanup`.
pub fn shared_precompare_cleanup(text: &str) -> String {
    let lowered = text.to_lowercase();
    let mut out = String::with_capacity(lowered.len());
    let mut chars = lowered.chars().peekable();

    while let Some(c) = chars.next() {
        // The `\\"` alternative consumes a backslash only when a quote follows it.
        if c == '\\' && chars.peek() == Some(&'"') {
            chars.next();
            continue;
        }
        if is_py_space(c) || matches!(c, '*' | '.' | ',' | ':' | '`' | '"' | '#' | '-') {
            continue;
        }
        out.push(c);
    }
    out
}

/// True for the Unicode blocks `_INITIAL_FILTER` removes.
#[inline]
fn is_filtered_block(c: char) -> bool {
    matches!(c as u32,
        0xfff0..=0xffff      // Specials
        | 0x1f000..=0x1f9ff  // Emoticons
        | 0x2000..=0x206f    // General Punctuation
        | 0x2190..=0x21ff    // Arrows
        | 0x2700..=0x27bf    // Dingbats
    )
}

/// Drop problematic Unicode blocks and control characters. Mirrors `clean_text`.
///
/// Note the Python source keeps only `\n` and `\t` below U+0020, so `\r` is dropped.
pub fn clean_text(text: &str) -> String {
    text.chars()
        .filter(|&c| !is_filtered_block(c))
        .filter(|&c| c >= ' ' || c == '\n' || c == '\t')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_runs_of_spaces_and_newlines() {
        // The space before `c` and `d` survives: only a space *followed by* a
        // line break is folded away.
        assert_eq!(
            strip_excessive_newlines_and_spaces("  a   b  \n\n\n c \r\n d  "),
            "a b\n c\n d"
        );
    }

    #[test]
    fn strips_newlines_to_single_space() {
        assert_eq!(strip_newlines("a\n\r\nb"), "a b");
    }

    #[test]
    fn precompare_drops_punctuation_and_escaped_quotes() {
        assert_eq!(
            shared_precompare_cleanup("Hello, World. *A*"),
            "helloworlda"
        );
        assert_eq!(shared_precompare_cleanup(r#"a\"b"#), "ab");
        // A lone backslash survives.
        assert_eq!(shared_precompare_cleanup(r"a\b"), r"a\b");
    }

    #[test]
    fn clean_text_drops_controls_but_keeps_tab_and_newline() {
        assert_eq!(clean_text("a\u{0}b\tc\nd\re"), "ab\tc\nde");
        assert_eq!(clean_text("a\u{2014}b"), "ab");
    }
}
