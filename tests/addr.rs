//! Tests for parsing server addresses (`xrcon::parse::parse_server_addr`).
//!
//! API under test:
//! `parse_server_addr(text: &str, default_port: u16) -> Result<(String, u16), _>`
//!
//! Accepted forms: `host`, `host:port`, `[ipv6]` and `[ipv6]:port`. The host
//! is returned without the square brackets. When the port is missing, the
//! given default port is returned.

use xrcon::parse::parse_server_addr;

/// Shorthand: the expected `(host, port)` result.
fn addr(host: &str, port: u16) -> (String, u16) {
    (host.to_string(), port)
}

// ---------------------------------------------------------------------------
// Valid addresses
// ---------------------------------------------------------------------------

/// A bare host name gets the default port (1234 here).
#[test]
fn hostname_without_port_uses_default() {
    assert_eq!(
        parse_server_addr("hostname", 1234).unwrap(),
        addr("hostname", 1234)
    );
}

/// A bare IPv6 address in brackets gets the default port and is returned
/// without the brackets.
#[test]
fn bracketed_ipv6_without_port_uses_default() {
    assert_eq!(parse_server_addr("[::1]", 6060).unwrap(), addr("::1", 6060));
}

/// A host name with a port returns that port, not the default.
#[test]
fn hostname_with_port() {
    assert_eq!(
        parse_server_addr("localhost:123", 26000).unwrap(),
        addr("localhost", 123)
    );
}

/// An IPv4 address with a port returns both parts.
#[test]
fn ipv4_with_port() {
    assert_eq!(
        parse_server_addr("127.0.0.1:26006", 26000).unwrap(),
        addr("127.0.0.1", 26006)
    );
}

/// A full IPv6 address in brackets with a port returns the address without
/// brackets and the port.
#[test]
fn bracketed_ipv6_with_port() {
    assert_eq!(
        parse_server_addr("[2001:db8:85a3:8d3:1319:8a2e:370:7348]:26006", 26000).unwrap(),
        addr("2001:db8:85a3:8d3:1319:8a2e:370:7348", 26006)
    );
}

/// A full IPv6 address in brackets without a port returns the default port
/// (26000 here).
#[test]
fn full_bracketed_ipv6_without_port_uses_default() {
    assert_eq!(
        parse_server_addr("[2001:db8:85a3:8d3:1319:8a2e:370:7348]", 26000).unwrap(),
        addr("2001:db8:85a3:8d3:1319:8a2e:370:7348", 26000)
    );
}

/// A bare IPv4 address gets the default port.
#[test]
fn ipv4_without_port_uses_default() {
    assert_eq!(
        parse_server_addr("192.168.1.10", 26000).unwrap(),
        addr("192.168.1.10", 26000)
    );
}

/// A host name with dots and dashes is accepted as a whole.
/// Source: reference code (any characters except `:` are allowed).
#[test]
fn hostname_with_dots_and_dashes() {
    assert_eq!(
        parse_server_addr("my-server.example.com:27000", 26000).unwrap(),
        addr("my-server.example.com", 27000)
    );
}

/// Upper-case hexadecimal digits in a bracketed IPv6 address are accepted
/// and kept as written.
#[test]
fn ipv6_uppercase_hex() {
    assert_eq!(
        parse_server_addr("[FE80::ABCD]:26000", 1).unwrap(),
        addr("FE80::ABCD", 26000)
    );
}

/// A given port replaces the default: the default is used only when the
/// address has no port.
#[test]
fn explicit_port_wins_over_default() {
    assert_eq!(parse_server_addr("host:1", 9999).unwrap(), addr("host", 1));
}

/// Leading zeros in the port are read as a decimal number: `0026` is 26.
#[test]
fn port_with_leading_zeros() {
    assert_eq!(parse_server_addr("host:0026", 1).unwrap(), addr("host", 26));
}

/// The largest valid port, 65535, is accepted.
#[test]
fn port_65535_is_accepted() {
    assert_eq!(
        parse_server_addr("host:65535", 1).unwrap(),
        addr("host", 65535)
    );
}

// ---------------------------------------------------------------------------
// Invalid ports
// ---------------------------------------------------------------------------

/// A port that is not all digits (`1d23`) is an error.
#[test]
fn non_numeric_port_is_rejected() {
    assert!(parse_server_addr("localhost:1d23", 26000).is_err());
}

/// Port `00` on a bracketed IPv6 address is an error, because it is zero.
#[test]
fn ipv6_port_00_is_rejected() {
    assert!(parse_server_addr("[::1]:00", 26000).is_err());
}

/// Port `0` on a host name is an error.
#[test]
fn port_zero_is_rejected() {
    assert!(parse_server_addr("host:0", 26000).is_err());
}

/// A port of 65536 does not fit into 16 bits and is an error.
#[test]
fn port_65536_is_rejected() {
    assert!(parse_server_addr("host:65536", 26000).is_err());
}

/// A very long number as the port is an error, not a panic or an overflow.
#[test]
fn huge_port_is_rejected() {
    assert!(parse_server_addr("host:99999999999999999999", 26000).is_err());
}

/// A negative port is an error: the port must be digits only.
#[test]
fn negative_port_is_rejected() {
    assert!(parse_server_addr("host:-1", 26000).is_err());
}

/// A trailing colon with no port digits is an error.
#[test]
fn trailing_colon_is_rejected() {
    assert!(parse_server_addr("host:", 26000).is_err());
}

/// A bracketed IPv6 address with a trailing colon and no port is an error.
#[test]
fn ipv6_trailing_colon_is_rejected() {
    assert!(parse_server_addr("[::1]:", 26000).is_err());
}

// ---------------------------------------------------------------------------
// Invalid hosts and shapes
// ---------------------------------------------------------------------------

/// An empty string is an error: a host is required.
#[test]
fn empty_string_is_rejected() {
    assert!(parse_server_addr("", 26000).is_err());
}

/// A port with no host (`:26000`) is an error.
#[test]
fn port_without_host_is_rejected() {
    assert!(parse_server_addr(":26000", 26000).is_err());
}

/// An IPv6 address without brackets (`::1`) is an error, because its colons
/// cannot be told apart from the port separator.
#[test]
fn unbracketed_ipv6_is_rejected() {
    assert!(parse_server_addr("::1", 26000).is_err());
}

/// An IPv6 address without the closing bracket is an error.
#[test]
fn missing_closing_bracket_is_rejected() {
    assert!(parse_server_addr("[::1", 26000).is_err());
}

/// Empty brackets are an error. The reference regex would accept `[]` as a
/// host name (its first alternative allows any text without a colon), so
/// this rejects something the reference lets through.
#[test]
fn empty_brackets_are_rejected() {
    assert!(parse_server_addr("[]", 26000).is_err());
}

/// Text after the closing bracket other than `:port` is an error.
#[test]
fn text_after_brackets_is_rejected() {
    assert!(parse_server_addr("[::1]x", 26000).is_err());
}

/// A host with two colons that is not bracketed (`a:b:1`) is an error.
#[test]
fn multiple_colons_without_brackets_are_rejected() {
    assert!(parse_server_addr("a:b:1", 26000).is_err());
}
