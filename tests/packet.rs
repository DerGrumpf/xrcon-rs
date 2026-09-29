//! Tests for building outgoing rcon packets (`xrcon::packet`).
//!
//! Every test builds a packet from fixed inputs and compares the resulting
//! bytes with an expected packet, or checks the packet's layout by slicing it.

use xrcon::packet::{rcon_nosecure_packet, rcon_secure_challenge_packet, rcon_secure_time_packet};

/// Every DarkPlaces out-of-band packet starts with four 0xFF bytes.
const HEADER: &[u8] = b"\xff\xff\xff\xff";
const TIME_PREFIX: &[u8] = b"srcon HMAC-MD4 TIME ";
const CHALLENGE_PREFIX: &[u8] = b"srcon HMAC-MD4 CHALLENGE ";
/// HMAC-MD4 digests are 16 bytes long.
const MAC_LEN: usize = 16;

/// Concatenates byte slices into one packet.
fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// Splits a secure packet into `(mac, tail)`, asserting that it starts with
/// `HEADER + prefix` and that the 16-byte MAC is followed by a space.
/// `tail` is everything after that space.
fn split_secure<'a>(packet: &'a [u8], prefix: &[u8]) -> (&'a [u8], &'a [u8]) {
    let head = [HEADER, prefix].concat();
    assert!(packet.starts_with(&head), "wrong header/prefix: {packet:?}");
    let rest = &packet[head.len()..];
    assert!(rest.len() > MAC_LEN, "packet too short: {packet:?}");
    let (mac, rest) = rest.split_at(MAC_LEN);
    assert_eq!(rest[0], b' ', "MAC must be followed by a space");
    (mac, &rest[1..])
}

// HMAC-MD4 digests (see the module docs for their origin).
const MAC_TIME_100_STATUS: [u8; 16] = [
    0x52, 0xcb, 0x76, 0xf0, 0xa7, 0x70, 0xcd, 0xca, 0xf2, 0x21, 0xc3, 0x7e, 0x06, 0xa9, 0x9f, 0xa8,
];
const MAC_TIME_1234_5_STATUS: [u8; 16] = [
    0xa1, 0x12, 0x05, 0xd5, 0xd7, 0x89, 0x93, 0x22, 0x8e, 0xfd, 0x19, 0xa7, 0x24, 0xef, 0x5e, 0x7b,
];
const MAC_TIME_100_SAY: [u8; 16] = [
    0x9e, 0x56, 0xcc, 0x89, 0x63, 0xc1, 0xd7, 0x62, 0xce, 0xc7, 0x88, 0xd3, 0x61, 0xd2, 0x37, 0x4a,
];
const MAC_TIME_100_STATUS_OTHER_PW: [u8; 16] = [
    0x06, 0xa8, 0x7d, 0x07, 0xee, 0x51, 0xc6, 0x3a, 0xf4, 0x6f, 0xd5, 0x0b, 0xda, 0x31, 0x9d, 0x82,
];
const MAC_TIME_0_STATUS: [u8; 16] = [
    0x06, 0xd8, 0x93, 0xe2, 0xeb, 0x63, 0x37, 0x56, 0x8d, 0x27, 0x58, 0x0b, 0x6e, 0x93, 0x3c, 0x45,
];
const MAC_CHAL_STATUS: [u8; 16] = [
    0x44, 0x89, 0xfd, 0x15, 0xcc, 0x5a, 0xea, 0xeb, 0x0e, 0xbf, 0x6c, 0xd6, 0x43, 0x05, 0x54, 0x12,
];
const MAC_CHAL_SAY: [u8; 16] = [
    0xb0, 0x7b, 0xd7, 0x5a, 0xb3, 0xd7, 0x21, 0x1d, 0x82, 0xce, 0xa8, 0x48, 0x17, 0x25, 0xc3, 0xe6,
];
const MAC_CHAL_STATUS_OTHER_PW: [u8; 16] = [
    0x55, 0x12, 0x87, 0xa9, 0x8a, 0xb2, 0x67, 0x39, 0xed, 0x05, 0x13, 0x76, 0x0f, 0xb7, 0x40, 0x38,
];

// ---------------------------------------------------------------------------
// Non-secure rcon: header + "rcon <password> <command>"
// ---------------------------------------------------------------------------

/// Builds a non-secure packet for password `passw` and command `status`
/// and compares it byte for byte with the expected packet.
#[test]
fn nosecure_reference_vector() {
    assert_eq!(
        rcon_nosecure_packet(b"passw", b"status"),
        cat(&[HEADER, b"rcon passw status"])
    );
}

/// A command containing spaces must be copied into the packet unchanged;
/// the builder must not split, quote or escape it.
#[test]
fn nosecure_command_with_spaces_is_unchanged() {
    assert_eq!(
        rcon_nosecure_packet(b"passw", b"say hello world"),
        cat(&[HEADER, b"rcon passw say hello world"])
    );
}

/// An empty command still produces `"rcon <password> "` with the trailing
/// space, because the format is a plain `"rcon {password} {command}"`.
#[test]
fn nosecure_empty_command_keeps_trailing_space() {
    assert_eq!(
        rcon_nosecure_packet(b"passw", b""),
        cat(&[HEADER, b"rcon passw "])
    );
}

/// Non-ASCII bytes in password and command must pass through as raw bytes.
/// This checks the byte-oriented API design (`&[u8]`); it is not a claim
/// about how the references encode text.
#[test]
fn nosecure_non_ascii_bytes_pass_through() {
    assert_eq!(
        rcon_nosecure_packet(b"p\xe4ss", b"say \xc3\xa4"),
        cat(&[HEADER, b"rcon p\xe4ss say \xc3\xa4"])
    );
}

// ---------------------------------------------------------------------------
// Secure time rcon:
// header + "srcon HMAC-MD4 TIME " + mac + " " + "<time> <command>"
// where mac = HMAC-MD4(key = password, data = "<time> <command>")
// and <time> is the timestamp printed with 6 decimals.
// ---------------------------------------------------------------------------

/// Builds a time-secure packet for timestamp 100.0, password `passw` and
/// command `status`, and compares it byte for byte with the expected packet.
#[test]
fn time_reference_vector() {
    let expected = cat(&[
        HEADER,
        TIME_PREFIX,
        &MAC_TIME_100_STATUS,
        b" 100.000000 status",
    ]);
    assert_eq!(
        rcon_secure_time_packet(b"passw", b"status", 100.0),
        expected
    );
}

/// A fractional timestamp (1234.5) must be printed as `1234.500000`
/// and be part of the signed data.
#[test]
fn time_fractional_timestamp() {
    let expected = cat(&[
        HEADER,
        TIME_PREFIX,
        &MAC_TIME_1234_5_STATUS,
        b" 1234.500000 status",
    ]);
    assert_eq!(
        rcon_secure_time_packet(b"passw", b"status", 1234.5),
        expected
    );
}

/// Timestamp 0.0 must be printed as `0.000000` (no padding, no trimming).
#[test]
fn time_zero_timestamp() {
    let expected = cat(&[HEADER, TIME_PREFIX, &MAC_TIME_0_STATUS, b" 0.000000 status"]);
    assert_eq!(rcon_secure_time_packet(b"passw", b"status", 0.0), expected);
}

/// A command with spaces is signed and sent as one string after the time.
#[test]
fn time_command_with_spaces() {
    let expected = cat(&[
        HEADER,
        TIME_PREFIX,
        &MAC_TIME_100_SAY,
        b" 100.000000 say hello world",
    ]);
    assert_eq!(
        rcon_secure_time_packet(b"passw", b"say hello world", 100.0),
        expected
    );
}

/// The password is the HMAC key: changing it (to `other`) changes only the
/// MAC bytes, not the readable tail.
#[test]
fn time_password_changes_mac() {
    let expected = cat(&[
        HEADER,
        TIME_PREFIX,
        &MAC_TIME_100_STATUS_OTHER_PW,
        b" 100.000000 status",
    ]);
    assert_eq!(
        rcon_secure_time_packet(b"other", b"status", 100.0),
        expected
    );
}

/// The password must never appear in the packet in clear text.
/// Checked by searching the whole packet for the password bytes.
#[test]
fn time_packet_does_not_contain_password() {
    let packet = rcon_secure_time_packet(b"hunter2-secret", b"status", 100.0);
    assert!(!packet.windows(14).any(|w| w == b"hunter2-secret"));
}

/// The timestamp is printed with exactly 6 decimals, rounded:
/// 1.0000004 becomes `1.000000` and 1.0000006 becomes `1.000001`.
/// Only the tail is checked; the MAC bytes are not compared.
#[test]
fn time_timestamp_has_six_decimals_rounded() {
    let p = rcon_secure_time_packet(b"passw", b"status", 1.0000004);
    let (_, tail) = split_secure(&p, TIME_PREFIX);
    assert_eq!(tail, b"1.000000 status");

    let p = rcon_secure_time_packet(b"passw", b"status", 1.0000006);
    let (_, tail) = split_secure(&p, TIME_PREFIX);
    assert_eq!(tail, b"1.000001 status");
}

/// The packet is exactly `header + prefix + 16-byte MAC + space + tail`;
/// checked by length and by slicing, without comparing the MAC bytes.
#[test]
fn time_packet_layout_and_length() {
    let p = rcon_secure_time_packet(b"passw", b"status", 100.0);
    let (_, tail) = split_secure(&p, TIME_PREFIX);
    assert_eq!(tail, b"100.000000 status");
    assert_eq!(
        p.len(),
        HEADER.len() + TIME_PREFIX.len() + MAC_LEN + 1 + tail.len()
    );
}

// ---------------------------------------------------------------------------
// Secure challenge rcon:
// header + "srcon HMAC-MD4 CHALLENGE " + mac + " " + challenge + " " + command
// where mac = HMAC-MD4(key = password, data = "<challenge> <command>").
// ---------------------------------------------------------------------------

/// Builds a challenge-secure packet for challenge `11111111111`, password
/// `passw` and command `status` and compares it byte for byte.
#[test]
fn challenge_reference_vector() {
    let expected = cat(&[
        HEADER,
        CHALLENGE_PREFIX,
        &MAC_CHAL_STATUS,
        b" 11111111111 status",
    ]);
    assert_eq!(
        rcon_secure_challenge_packet(b"passw", b"11111111111", b"status"),
        expected
    );
}

/// A command with spaces is signed and sent as one string after the
/// challenge.
#[test]
fn challenge_command_with_spaces() {
    let expected = cat(&[
        HEADER,
        CHALLENGE_PREFIX,
        &MAC_CHAL_SAY,
        b" 11111111111 say hello world",
    ]);
    assert_eq!(
        rcon_secure_challenge_packet(b"passw", b"11111111111", b"say hello world"),
        expected
    );
}

/// The password is the HMAC key: changing it (to `other`) changes only the
/// MAC bytes.
#[test]
fn challenge_password_changes_mac() {
    let expected = cat(&[
        HEADER,
        CHALLENGE_PREFIX,
        &MAC_CHAL_STATUS_OTHER_PW,
        b" 11111111111 status",
    ]);
    assert_eq!(
        rcon_secure_challenge_packet(b"other", b"11111111111", b"status"),
        expected
    );
}

/// The password must never appear in the packet in clear text.
#[test]
fn challenge_packet_does_not_contain_password() {
    let packet = rcon_secure_challenge_packet(b"hunter2-secret", b"11111111111", b"status");
    assert!(!packet.windows(14).any(|w| w == b"hunter2-secret"));
}

/// The packet is exactly `header + prefix + 16-byte MAC + space + challenge
/// + space + command`; checked by length and slicing, MAC bytes not compared.
#[test]
fn challenge_packet_layout_and_length() {
    let p = rcon_secure_challenge_packet(b"passw", b"11111111111", b"status");
    let (_, tail) = split_secure(&p, CHALLENGE_PREFIX);
    assert_eq!(tail, b"11111111111 status");
    assert_eq!(
        p.len(),
        HEADER.len() + CHALLENGE_PREFIX.len() + MAC_LEN + 1 + tail.len()
    );
}
