//! Tests for parsing incoming challenge and rcon replies (`xrcon::parse`).
//!
//! Every test feeds one raw packet to a parser and checks either the returned
//! bytes or that the packet is rejected (`Err`).

use xrcon::parse::{parse_challenge_response, parse_rcon_response};

/// Every DarkPlaces out-of-band packet starts with four 0xFF bytes.
const HEADER: &[u8] = b"\xff\xff\xff\xff";

/// Builds a challenge reply: header + `challenge ` + `body`.
fn challenge_packet(body: &[u8]) -> Vec<u8> {
    [HEADER, b"challenge ", body].concat()
}

/// Builds an rcon reply: header + `n` + `body`.
fn rcon_packet(body: &[u8]) -> Vec<u8> {
    [HEADER, b"n", body].concat()
}

// ---------------------------------------------------------------------------
// Challenge reply: header + "challenge " + 11-byte challenge + anything
// ---------------------------------------------------------------------------

/// Parses the full challenge reply  test (a real
/// server-style packet with a NUL byte and extra data after the challenge)
/// and checks that only the 11 challenge bytes are returned.
#[test]
fn challenge_reference_vector() {
    let packet = b"\xff\xff\xff\xffchallenge 11111111111\x00vlen.\x00\x00\x00d0pkXon//KssdlzGkFKdnnN4sgg8H+koTbBn5JTi37BAW1t=\x00\x00";
    assert_eq!(
        parse_challenge_response(packet).unwrap(),
        &b"11111111111"[..]
    );
}

/// A reply that ends right after the 11 challenge bytes is valid: the NUL
/// byte and the extra data are not required.
#[test]
fn challenge_exactly_eleven_bytes_and_nothing_else() {
    let packet = challenge_packet(b"abcdefghijk");
    assert_eq!(
        parse_challenge_response(&packet).unwrap(),
        &b"abcdefghijk"[..]
    );
}

/// A reply with the usual NUL terminator right after the challenge returns
/// the challenge without the NUL.
#[test]
fn challenge_followed_by_nul() {
    let packet = challenge_packet(b"abcdefghijk\x00");
    assert_eq!(
        parse_challenge_response(&packet).unwrap(),
        &b"abcdefghijk"[..]
    );
}

/// Only the first 11 bytes after the header are the challenge; whatever
/// follows is ignored, even if it looks like more challenge characters.
#[test]
fn challenge_extra_bytes_are_ignored() {
    let packet = challenge_packet(b"12345678901234567890");
    assert_eq!(
        parse_challenge_response(&packet).unwrap(),
        &b"12345678901"[..]
    );
}

/// The challenge may contain punctuation. The engine draws characters from
/// `!` to `~` except `\`, `;`, `"`, `%` and `/`, so the 11 bytes
/// `~!@#$&*()+=` are possible, and the parser must return them unchanged.
#[test]
fn challenge_with_punctuation_is_returned_verbatim() {
    let packet = challenge_packet(b"~!@#$&*()+=\x00");
    assert_eq!(
        parse_challenge_response(&packet).unwrap(),
        &b"~!@#$&*()+="[..]
    );
}

/// A challenge with 10 bytes after the header (one byte too short) is an error.
#[test]
fn challenge_ten_bytes_is_rejected() {
    assert!(parse_challenge_response(&challenge_packet(b"abcdefghij")).is_err());
}

/// A 5-character challenge followed by a NUL byte, with nothing after it, is
/// an error: fewer than 11 bytes follow the header.
#[test]
fn challenge_short_and_terminated_is_rejected() {
    assert!(parse_challenge_response(&challenge_packet(b"abcde\x00")).is_err());
}

/// A reply with the `challenge ` prefix and no challenge bytes at all is an
/// error.
#[test]
fn challenge_prefix_only_is_rejected() {
    assert!(parse_challenge_response(&challenge_packet(b"")).is_err());
}

/// The text `bad packet`, which has neither the 0xFF header nor the
/// `challenge ` prefix, is an error.
#[test]
fn challenge_bad_packet_is_rejected() {
    assert!(parse_challenge_response(b"bad packet").is_err());
}

/// An empty packet is an error.
#[test]
fn challenge_empty_packet_is_rejected() {
    assert!(parse_challenge_response(b"").is_err());
}

/// The `challenge ` text without the four 0xFF header bytes is an error, even
/// though the rest of the packet is well formed.
#[test]
fn challenge_missing_ff_header_is_rejected() {
    assert!(parse_challenge_response(b"challenge 11111111111").is_err());
}

/// A header with only three 0xFF bytes is an error.
#[test]
fn challenge_short_ff_header_is_rejected() {
    assert!(parse_challenge_response(b"\xff\xff\xffchallenge 11111111111").is_err());
}

/// `challenge` without the space that ends the prefix is an error: the
/// prefix is `challenge ` including the space.
#[test]
fn challenge_prefix_without_space_is_rejected() {
    assert!(parse_challenge_response(b"\xff\xff\xff\xffchallenge11111111111").is_err());
}

/// An rcon reply (`n` prefix) whose body contains the text `challenge ` is
/// not a challenge reply.
#[test]
fn challenge_parser_rejects_rcon_reply() {
    assert!(parse_challenge_response(&rcon_packet(b"challenge 11111111111")).is_err());
}

// ---------------------------------------------------------------------------
// Rcon reply: header + "n" + body
// ---------------------------------------------------------------------------

/// Parses `header + n + Test` and checks that the body `Test` is returned.
#[test]
fn rcon_reference_vector() {
    assert_eq!(
        parse_rcon_response(&rcon_packet(b"Test")).unwrap(),
        &b"Test"[..]
    );
}

/// A reply with an empty body is valid and returns an empty slice.
#[test]
fn rcon_empty_body() {
    assert_eq!(parse_rcon_response(&rcon_packet(b"")).unwrap(), &b""[..]);
}

/// Only the single `n` after the header is removed: a body that itself starts
/// with `n` keeps that letter.
#[test]
fn rcon_body_starting_with_n_keeps_it() {
    assert_eq!(
        parse_rcon_response(&rcon_packet(b"nn")).unwrap(),
        &b"nn"[..]
    );
}

/// The body is returned unchanged: newlines, a NUL byte, 0xFF bytes and
/// trailing whitespace are all kept (no trimming, no decoding).
#[test]
fn rcon_body_is_returned_verbatim() {
    let body = b"line1\nline2\r\n\x00\xff\xff \t ";
    assert_eq!(parse_rcon_response(&rcon_packet(body)).unwrap(), &body[..]);
}

/// The text `bad packet` is an error.
#[test]
fn rcon_bad_packet_is_rejected() {
    assert!(parse_rcon_response(b"bad packet").is_err());
}

/// An empty packet is an error.
#[test]
fn rcon_empty_packet_is_rejected() {
    assert!(parse_rcon_response(b"").is_err());
}

/// The four 0xFF bytes alone, without the `n`, are an error.
#[test]
fn rcon_header_without_n_is_rejected() {
    assert!(parse_rcon_response(HEADER).is_err());
}

/// `nTest` without the four 0xFF header bytes is an error.
#[test]
fn rcon_missing_ff_header_is_rejected() {
    assert!(parse_rcon_response(b"nTest").is_err());
}

/// A header with only three 0xFF bytes is an error.
#[test]
fn rcon_short_ff_header_is_rejected() {
    assert!(parse_rcon_response(b"\xff\xff\xffnTest").is_err());
}

/// A challenge reply (prefix `challenge `, not `n`) is not an rcon reply.
#[test]
fn rcon_parser_rejects_challenge_reply() {
    assert!(parse_rcon_response(&challenge_packet(b"11111111111\x00")).is_err());
}

/// A status reply (`statusResponse`) is not an rcon reply.
#[test]
fn rcon_parser_rejects_status_reply() {
    assert!(parse_rcon_response(b"\xff\xff\xff\xffstatusResponse\n\\gamename\\Xonotic\n").is_err());
}
