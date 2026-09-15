//! Decoding a request body the way Python's `json.loads` does.
//!
//! `json.loads` accepts *bytes*, and when it does it runs
//! `json.detect_encoding` first: it honours a UTF-8, UTF-16 or UTF-32 byte
//! order mark, and infers UTF-16/UTF-32 from the null-byte pattern of the first
//! four bytes even without one.
//!
//! The matcher has to do the same. A body the Python gate recognises and this
//! one does not falls through to the whole-domain `ASK` — which means an
//! admin's explicit `DENY` on the invoked action is never consulted. Encoding a
//! GraphQL body as UTF-16 would be enough to turn a block into a prompt.

/// Python's `json.detect_encoding`, as an explicit decode.
///
/// Returns `None` when the bytes do not decode under the detected encoding,
/// which is what `UnicodeDecodeError` becomes on the Python side — a body that
/// simply matches no rule.
pub fn decode(body: &[u8]) -> Option<String> {
    const BOM_UTF32_BE: &[u8] = &[0x00, 0x00, 0xFE, 0xFF];
    const BOM_UTF32_LE: &[u8] = &[0xFF, 0xFE, 0x00, 0x00];
    const BOM_UTF16_BE: &[u8] = &[0xFE, 0xFF];
    const BOM_UTF16_LE: &[u8] = &[0xFF, 0xFE];
    const BOM_UTF8: &[u8] = &[0xEF, 0xBB, 0xBF];

    // The UTF-32 BOMs are checked first: the little-endian one starts with the
    // UTF-16 little-endian BOM, so the other order would mis-detect it.
    if let Some(rest) = body.strip_prefix(BOM_UTF32_BE) {
        return decode_utf32(rest, Endian::Big);
    }
    if let Some(rest) = body.strip_prefix(BOM_UTF32_LE) {
        return decode_utf32(rest, Endian::Little);
    }
    if let Some(rest) = body.strip_prefix(BOM_UTF16_BE) {
        return decode_utf16(rest, Endian::Big);
    }
    if let Some(rest) = body.strip_prefix(BOM_UTF16_LE) {
        return decode_utf16(rest, Endian::Little);
    }
    if let Some(rest) = body.strip_prefix(BOM_UTF8) {
        return String::from_utf8(rest.to_vec()).ok();
    }

    // No BOM: infer from where the null bytes fall in the first four bytes.
    // JSON's first character is always ASCII, so the pattern is unambiguous.
    if body.len() >= 4 {
        if body[0] == 0 {
            return if body[1] != 0 {
                decode_utf16(body, Endian::Big)
            } else {
                decode_utf32(body, Endian::Big)
            };
        }
        if body[1] == 0 {
            return if body[2] != 0 || body[3] != 0 {
                decode_utf16(body, Endian::Little)
            } else {
                decode_utf32(body, Endian::Little)
            };
        }
    } else if body.len() == 2 {
        if body[0] == 0 {
            return decode_utf16(body, Endian::Big);
        }
        if body[1] == 0 {
            return decode_utf16(body, Endian::Little);
        }
    }

    String::from_utf8(body.to_vec()).ok()
}

#[derive(Clone, Copy)]
enum Endian {
    Big,
    Little,
}

fn decode_utf16(bytes: &[u8], endian: Endian) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None; // a truncated code unit
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| match endian {
            Endian::Big => u16::from_be_bytes([pair[0], pair[1]]),
            Endian::Little => u16::from_le_bytes([pair[0], pair[1]]),
        })
        .collect();
    String::from_utf16(&units).ok()
}

fn decode_utf32(bytes: &[u8], endian: Endian) -> Option<String> {
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    bytes
        .chunks_exact(4)
        .map(|quad| {
            let raw = match endian {
                Endian::Big => u32::from_be_bytes([quad[0], quad[1], quad[2], quad[3]]),
                Endian::Little => u32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]),
            };
            char::from_u32(raw)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{"query":"mutation { issueCreate { id } }"}"#;

    fn utf16(text: &str, endian: Endian) -> Vec<u8> {
        text.encode_utf16()
            .flat_map(|unit| match endian {
                Endian::Big => unit.to_be_bytes(),
                Endian::Little => unit.to_le_bytes(),
            })
            .collect()
    }

    fn utf32(text: &str, endian: Endian) -> Vec<u8> {
        text.chars()
            .flat_map(|character| match endian {
                Endian::Big => (character as u32).to_be_bytes(),
                Endian::Little => (character as u32).to_le_bytes(),
            })
            .collect()
    }

    #[test]
    fn plain_utf8_is_unchanged() {
        assert_eq!(decode(JSON.as_bytes()).as_deref(), Some(JSON));
    }

    #[test]
    fn a_utf8_byte_order_mark_is_stripped() {
        // The case a real corpus run caught: without this the body parses as
        // nothing, the action is not recognised, and an admin's DENY on it is
        // never consulted.
        let mut body = vec![0xEF, 0xBB, 0xBF];
        body.extend_from_slice(JSON.as_bytes());
        assert_eq!(decode(&body).as_deref(), Some(JSON));
    }

    #[test]
    fn utf16_is_decoded_with_and_without_a_byte_order_mark() {
        for endian in [Endian::Big, Endian::Little] {
            assert_eq!(decode(&utf16(JSON, endian)).as_deref(), Some(JSON));

            let mut with_bom = utf16("\u{feff}", endian);
            with_bom.extend_from_slice(&utf16(JSON, endian));
            assert_eq!(decode(&with_bom).as_deref(), Some(JSON));
        }
    }

    #[test]
    fn utf32_is_decoded_with_and_without_a_byte_order_mark() {
        for endian in [Endian::Big, Endian::Little] {
            assert_eq!(decode(&utf32(JSON, endian)).as_deref(), Some(JSON));

            let mut with_bom = utf32("\u{feff}", endian);
            with_bom.extend_from_slice(&utf32(JSON, endian));
            assert_eq!(decode(&with_bom).as_deref(), Some(JSON));
        }
    }

    #[test]
    fn a_utf32_little_endian_mark_is_not_read_as_utf16() {
        // BOM_UTF32_LE starts with BOM_UTF16_LE, so the check order is load
        // bearing rather than cosmetic.
        let mut body = utf32("\u{feff}", Endian::Little);
        body.extend_from_slice(&utf32(JSON, Endian::Little));
        assert_eq!(decode(&body).as_deref(), Some(JSON));
    }

    #[test]
    fn a_two_byte_body_is_still_classified() {
        assert_eq!(decode(&[0x00, 0x7B]).as_deref(), Some("{"));
        assert_eq!(decode(&[0x7B, 0x00]).as_deref(), Some("{"));
    }

    #[test]
    fn undecodable_bytes_decode_to_nothing() {
        // Python raises UnicodeDecodeError, a ValueError, which the matcher
        // already treats as "this body matches no rule".
        assert!(decode(&[0xFF, 0xFE, 0x00]).is_none()); // truncated UTF-16
        assert!(decode(&[0x7B, 0x22, 0xC3, 0x28]).is_none()); // invalid UTF-8
    }

    #[test]
    fn an_empty_body_decodes_to_an_empty_string() {
        assert_eq!(decode(b"").as_deref(), Some(""));
    }
}
