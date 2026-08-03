//! Percent-encoding for the dynamic parts of a request path.
//!
//! Symbols go into paths, and symbols are not path-safe. `BTC/USD` carries a
//! separator, a future option symbol such as `./ESZ4 EW4U4 240927P5520`
//! carries two spaces and a separator, and any of them could one day carry a
//! `?` or a `#`. Interpolating one of those raw does not produce a failed
//! request — it produces a **different** request, against whatever route the
//! extra separators happen to select.
//!
//! Before this module each call site made its own arrangement: three encoded
//! `/`, one of those also encoded `.` and a space, and eleven encoded nothing.
//! That is the shape a transport concern takes when it is solved per endpoint,
//! and the next endpoint inherits whichever variant its author copied.
//!
//! Query strings are a separate problem with a separate solution: they are
//! encoded by the HTTP client's serializer in
//! [`crate::TastyTrade::get_with_query`], and nothing here touches them.

/// Percent-encodes one path segment per RFC 3986.
///
/// Everything outside the unreserved set — `ALPHA`, `DIGIT`, `-`, `.`, `_`,
/// `~` — becomes `%XX` over the UTF-8 bytes, uppercase, which is what RFC 3986
/// §2.1 says a percent-encoded octet looks like. Encoding the whole complement
/// rather than an enumerated deny list is the point: a symbol containing a
/// character nobody anticipated is encoded because it was never on the keep
/// list, instead of passing through because it was never on the deny list.
///
/// **One segment**, never a path. Handing this a whole path encodes its
/// separators and produces a single nonsense segment; the caller joins the
/// encoded pieces with `/` itself.
///
/// It also must be applied exactly once. `%` is outside the unreserved set, so
/// encoding an already-encoded value turns `%2F` into `%252F` — which is why
/// the manual `.replace("/", "%2F")` calls this replaces were removed rather
/// than left in place beneath it.
///
/// ```ignore
/// assert_eq!(encode_path_segment("BTC/USD"), "BTC%2FUSD");
/// assert_eq!(encode_path_segment("SPY"), "SPY");
/// ```
pub(crate) fn encode_path_segment(segment: &str) -> String {
    // Most segments are already clean, and the common case should not
    // reallocate: an ordinary equity symbol comes back byte for byte.
    let mut encoded = String::with_capacity(segment.len());

    for byte in segment.as_bytes() {
        if is_unreserved(*byte) {
            encoded.push(*byte as char);
        } else {
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }

    encoded
}

/// Uppercase because RFC 3986 §6.2.2.1 says producers should use it.
const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// RFC 3986 §2.3: `ALPHA / DIGIT / "-" / "." / "_" / "~"`.
///
/// Deliberately the *unreserved* set and not the larger `pchar` set. `pchar`
/// also permits `:`, `@` and the sub-delimiters `!$&'()*+,;=`, all of which are
/// legal in a segment — but a symbol containing one carries no meaning that
/// depends on it surviving unencoded, and an encoded octet decodes back to the
/// same character. Choosing the smaller set costs nothing and removes a row of
/// judgement calls.
fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The characters that change which endpoint is selected. A `/` that
    /// survives is a different route; a `?` or `#` that survives ends the path
    /// entirely and turns the rest of the symbol into a query or a fragment.
    #[test]
    fn a_separator_can_never_survive_into_the_path() {
        assert_eq!(encode_path_segment("BTC/USD"), "BTC%2FUSD");
        assert_eq!(encode_path_segment("a?b"), "a%3Fb");
        assert_eq!(encode_path_segment("a#b"), "a%23b");
        assert_eq!(encode_path_segment("a b"), "a%20b");
    }

    /// The unreserved set passes through unchanged. RFC 3986 §2.3 says
    /// producers should not encode it, and a fixture or a log line reading
    /// `SPY` rather than `%53%50%59` is worth having.
    #[test]
    fn an_ordinary_symbol_is_left_alone() {
        assert_eq!(encode_path_segment("SPY"), "SPY");
        assert_eq!(encode_path_segment("BRK.B"), "BRK.B");
        assert_eq!(encode_path_segment("a-b_c~d.9"), "a-b_c~d.9");
        assert_eq!(encode_path_segment("5WX12345"), "5WX12345");
    }

    /// The real symbols this was written for.
    #[test]
    fn the_symbols_that_motivated_this_round_trip() {
        // Equity with a class separator.
        assert_eq!(encode_path_segment("BRK/B"), "BRK%2FB");
        // Futures: the leading slash is part of the symbol.
        assert_eq!(encode_path_segment("/ESZ4"), "%2FESZ4");
        // A future option: a leading dot-slash and two spaces.
        assert_eq!(
            encode_path_segment("./ESZ4 EW4U4 240927P5520"),
            ".%2FESZ4%20EW4U4%20240927P5520"
        );
    }

    /// `%` is not unreserved, so a literal one in the input is encoded. This
    /// is what makes the function safe to apply to raw user input — and what
    /// makes applying it twice wrong.
    #[test]
    fn a_percent_in_the_input_is_encoded_rather_than_trusted() {
        assert_eq!(encode_path_segment("100%"), "100%25");
        // Applied twice, `%2F` becomes `%252F`. Asserted rather than merely
        // documented, because the failure is silent: the request succeeds
        // against a route nobody meant to call.
        assert_eq!(
            encode_path_segment(&encode_path_segment("BTC/USD")),
            "BTC%252FUSD"
        );
    }

    /// Non-ASCII is encoded per UTF-8 byte, which is what RFC 3986 §2.5
    /// requires and what every server this crate talks to expects.
    #[test]
    fn non_ascii_is_encoded_byte_by_byte() {
        assert_eq!(encode_path_segment("ñ"), "%C3%B1");
        assert_eq!(encode_path_segment("€"), "%E2%82%AC");
        // A multi-byte character must not be split or lost.
        assert_eq!(encode_path_segment("a€b"), "a%E2%82%ACb");
    }

    /// Hex digits are uppercase per RFC 3986 §6.2.2.1, so two encoders of the
    /// same value produce the same string and a test can compare them.
    #[test]
    fn hex_digits_are_uppercase() {
        assert_eq!(encode_path_segment("\u{7f}"), "%7F");
        assert_eq!(encode_path_segment("["), "%5B");
    }

    /// An empty segment stays empty rather than becoming something. The venue
    /// answers 404 for it, which is the right answer to a request for nothing.
    #[test]
    fn an_empty_segment_encodes_to_nothing() {
        assert_eq!(encode_path_segment(""), "");
    }

    /// The old per-endpoint encoding must not come back. Three call sites each
    /// had their own `.replace(...)` chain and eight more had none, which is
    /// exactly the failure this module exists to end — and a reviewer adding a
    /// twelfth endpoint has no reason to know that.
    #[test]
    fn no_endpoint_rolls_its_own_encoding() {
        for (name, source) in [
            ("accounts.rs", include_str!("accounts.rs")),
            ("client.rs", include_str!("client.rs")),
            ("instrument.rs", include_str!("instrument.rs")),
            ("option_chain.rs", include_str!("option_chain.rs")),
            ("quote_streaming.rs", include_str!("quote_streaming.rs")),
        ] {
            assert!(
                !source.contains(r#".replace("/""#),
                "{name} encodes a path separator by hand; use encode_path_segment"
            );
            assert!(
                !source.contains("%2F") && !source.contains("%2f"),
                "{name} writes a percent-escape by hand; use encode_path_segment"
            );
        }
    }
}
