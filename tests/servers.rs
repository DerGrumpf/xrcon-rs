//! Tests for parsing master-server replies (`xrcon::parse::parse_servers_response`).
//!
//! A reply is `header + "getserversResponse"`, followed by one 7-byte entry
//! per server (`\` + 4 IPv4 bytes + 2 port bytes, big-endian), and ends with
//! the 7-byte terminator `\EOT\0\0\0`.

use std::net::Ipv4Addr;

use xrcon::parse::parse_servers_response;

/// Every DarkPlaces out-of-band packet starts with four 0xFF bytes.
const HEADER: &[u8] = b"\xff\xff\xff\xff";
/// The text that follows the header in a master-server reply.
const RESPONSE_TEXT: &[u8] = b"getserversResponse";
/// The 7-byte block that ends the server list.
const EOT: &[u8] = b"\\EOT\x00\x00\x00";

/// Builds one 7-byte server entry: `\`, the 4 address bytes, then the port
/// in big-endian order.
fn entry(ip: [u8; 4], port: u16) -> Vec<u8> {
    [b"\\".as_slice(), &ip, &port.to_be_bytes()].concat()
}

/// Builds a reply from header, response text, the given entries, and the
/// terminator.
fn reply(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut packet = [HEADER, RESPONSE_TEXT].concat();
    for e in entries {
        packet.extend_from_slice(e);
    }
    packet.extend_from_slice(EOT);
    packet
}

// ---------------------------------------------------------------------------
// Valid replies
// ---------------------------------------------------------------------------

/// Parses the reference packet, written byte for byte
/// (three servers and the terminator), and checks the three addresses and
/// ports in packet order.
#[test]
fn reference_vector() {
    let packet = b"\xff\xff\xff\xffgetserversResponse\\\x82\x957\x16e\xbd\\\xc8+\xc0|e\x92\\zcv\x05e\x91\\EOT\x00\x00\x00";
    assert_eq!(
        parse_servers_response(packet).unwrap(),
        vec![
            (Ipv4Addr::new(130, 149, 55, 22), 26045),
            (Ipv4Addr::new(200, 43, 192, 124), 26002),
            (Ipv4Addr::new(122, 99, 118, 5), 26001),
        ]
    );
}

/// A reply with only the terminator is valid and returns an empty list.
#[test]
fn empty_list() {
    assert_eq!(parse_servers_response(&reply(&[])).unwrap(), vec![]);
}

/// A reply with a single server returns exactly that address and port.
/// Source: reference code.
#[test]
fn single_server() {
    let packet = reply(&[entry([127, 0, 0, 1], 26000)]);
    assert_eq!(
        parse_servers_response(&packet).unwrap(),
        vec![(Ipv4Addr::new(127, 0, 0, 1), 26000)]
    );
}

/// The port is read as a big-endian 16-bit number: bytes `0x65 0xbd` are
/// 26045, not 48485.
#[test]
fn port_is_big_endian() {
    let packet = reply(&[entry([1, 2, 3, 4], 0x65bd)]);
    assert_eq!(parse_servers_response(&packet).unwrap()[0].1, 26045);
}

/// The smallest and largest port values (1 and 65535) are returned unchanged.
#[test]
fn port_extremes() {
    let packet = reply(&[entry([10, 0, 0, 1], 1), entry([10, 0, 0, 2], 65535)]);
    assert_eq!(
        parse_servers_response(&packet).unwrap(),
        vec![
            (Ipv4Addr::new(10, 0, 0, 1), 1),
            (Ipv4Addr::new(10, 0, 0, 2), 65535),
        ]
    );
}

/// Address bytes that look like ASCII characters (`\`, `E`, `O`, `T`) are
/// still just address bytes: the entry `\` + `E`,`O`,`T`,`\` + port is a
/// server, not a terminator.
#[test]
fn address_bytes_that_look_like_text() {
    let packet = reply(&[entry([b'E', b'O', b'T', b'\\'], 26000)]);
    assert_eq!(
        parse_servers_response(&packet).unwrap(),
        vec![(Ipv4Addr::new(69, 79, 84, 92), 26000)]
    );
}

/// Many entries (100) are all returned, in packet order.
#[test]
fn many_servers_keep_packet_order() {
    let entries: Vec<Vec<u8>> = (0..100u8)
        .map(|i| entry([10, 0, 0, i], 26000 + u16::from(i)))
        .collect();
    let got = parse_servers_response(&reply(&entries)).unwrap();
    assert_eq!(got.len(), 100);
    for (i, (ip, port)) in got.iter().enumerate() {
        assert_eq!(*ip, Ipv4Addr::new(10, 0, 0, i as u8));
        assert_eq!(*port, 26000 + i as u16);
    }
}

/// Bytes after the terminator are ignored: parsing stops at `\EOT\0\0\0`.
#[test]
fn data_after_terminator_is_ignored() {
    let mut packet = reply(&[entry([127, 0, 0, 1], 26000)]);
    packet.extend_from_slice(b"garbage after the end");
    assert_eq!(
        parse_servers_response(&packet).unwrap(),
        vec![(Ipv4Addr::new(127, 0, 0, 1), 26000)]
    );
}

/// Entries after a terminator are not returned: a terminator in the middle
/// of the list ends it.
#[test]
fn entries_after_terminator_are_not_returned() {
    let mut packet = reply(&[entry([127, 0, 0, 1], 26000)]);
    packet.extend_from_slice(&entry([127, 0, 0, 2], 26001));
    packet.extend_from_slice(EOT);
    assert_eq!(
        parse_servers_response(&packet).unwrap(),
        vec![(Ipv4Addr::new(127, 0, 0, 1), 26000)]
    );
}

// ---------------------------------------------------------------------------
// Invalid replies: list structure
// ---------------------------------------------------------------------------

/// A list that ends without the terminator is an error.
#[test]
fn missing_terminator_is_rejected() {
    let packet = [HEADER, RESPONSE_TEXT, &entry([122, 99, 118, 5], 26001)].concat();
    assert!(parse_servers_response(&packet).is_err());
}

/// A header with no entries and no terminator is an error.
#[test]
fn header_only_is_rejected() {
    let packet = [HEADER, RESPONSE_TEXT].concat();
    assert!(parse_servers_response(&packet).is_err());
}

/// An entry that does not start with a backslash is an error, even when the
/// terminator follows.
#[test]
fn entry_without_backslash_is_rejected() {
    let mut bad = entry([122, 99, 118, 5], 26001);
    bad[0] = b't';
    let packet = [HEADER, RESPONSE_TEXT, &bad, EOT].concat();
    assert!(parse_servers_response(&packet).is_err());
}

/// A last entry cut short (fewer than 7 bytes) and no terminator is an error.
#[test]
fn truncated_last_entry_is_rejected() {
    let full = entry([122, 99, 118, 5], 26001);
    let packet = [HEADER, RESPONSE_TEXT, &full[..5]].concat();
    assert!(parse_servers_response(&packet).is_err());
}

/// A terminator cut short (`\EOT` only) is an error: it is neither a
/// terminator nor a complete entry.
#[test]
fn truncated_terminator_is_rejected() {
    let packet = [HEADER, RESPONSE_TEXT, b"\\EOT".as_slice()].concat();
    assert!(parse_servers_response(&packet).is_err());
}

/// Valid entries followed by a bad entry, and no terminator, are an error:
/// the valid entries are not returned as a partial result.
#[test]
fn bad_entry_after_valid_entries_is_rejected() {
    let mut bad = entry([1, 2, 3, 4], 26000);
    bad[0] = b'x';
    let packet = [HEADER, RESPONSE_TEXT, &entry([127, 0, 0, 1], 26000), &bad].concat();
    assert!(parse_servers_response(&packet).is_err());
}

// ---------------------------------------------------------------------------
// Invalid replies: header
// ---------------------------------------------------------------------------

/// An empty packet is an error.
#[test]
fn empty_packet_is_rejected() {
    assert!(parse_servers_response(b"").is_err());
}

/// A reply without the four 0xFF bytes is an error even if the rest is
/// valid.
#[test]
fn missing_ff_header_is_rejected() {
    let packet = [RESPONSE_TEXT, &entry([127, 0, 0, 1], 26000), EOT].concat();
    assert!(parse_servers_response(&packet).is_err());
}

/// A header with only three 0xFF bytes is an error.
#[test]
fn short_ff_header_is_rejected() {
    let packet = [
        b"\xff\xff\xff".as_slice(),
        RESPONSE_TEXT,
        &entry([127, 0, 0, 1], 26000),
        EOT,
    ]
    .concat();
    assert!(parse_servers_response(&packet).is_err());
}

/// A response text that differs by one letter (`getserversRespons`) is an
/// error even if the entries are valid.
#[test]
fn wrong_response_text_is_rejected() {
    let packet = [
        HEADER,
        b"getserversRespons".as_slice(),
        &entry([127, 0, 0, 1], 26000),
        EOT,
    ]
    .concat();
    assert!(parse_servers_response(&packet).is_err());
}

/// A status reply is not a master-server reply.
#[test]
fn status_reply_is_rejected() {
    assert!(
        parse_servers_response(b"\xff\xff\xff\xffstatusResponse\n\\gamename\\Xonotic\n").is_err()
    );
}
