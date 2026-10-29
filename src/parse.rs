//! Parsing incoming packets and addresses.
//!
//! Every packet parser checks the header for its packet type first and
//! returns [`Error::BadHeader`] on mismatch. No function here panics on
//! any input.

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::packet::HEADER;
use crate::{Error, Result};

/// Terminator of a master-server reply: `\EOT` followed by three NUL bytes.
const SERVERS_TERMINATOR: [u8; 7] = *b"\\EOT\0\0\0";

/// Strips `H` and then `tag` from the start of `packet`.
fn strip_header<'a>(packet: &'a [u8], tag: &[u8]) -> Result<&'a [u8]> {
    packet
        .strip_prefix(&HEADER[..])
        .and_then(|rest| rest.strip_prefix(tag))
        .ok_or(Error::BadHeader)
}

/// Parses a challenge reply (`H "challenge " <challenge> ...`).
///
/// Returns the 11 bytes after `"challenge "`. Bytes after the 11th are
/// ignored. The content of the 11 bytes is not validated.
pub fn parse_challenge_response(packet: &[u8]) -> Result<&[u8]> {
    let rest = strip_header(packet, b"challenge ")?;
    rest.get(..11).ok_or(Error::ChallengeTooShort)
}

/// Parses an rcon reply (`H "n" <body>`).
///
/// Returns the body byte for byte. An empty body is valid.
pub fn parse_rcon_response(packet: &[u8]) -> Result<&[u8]> {
    strip_header(packet, b"n")
}

/// Parses a server address into host and port.
///
/// Accepts `host`, `host:port`, `[ipv6]` and `[ipv6]:port`. Brackets are
/// removed from the returned host. A missing port yields `default_port`.
/// The port must be decimal digits only (leading zeros allowed) in
/// `1..=65535`. The host text itself is not validated or trimmed.
pub fn parse_server_addr(text: &str, default_port: u16) -> Result<(String, u16)> {
    if text.is_empty() {
        return Err(Error::EmptyAddr);
    }

    if let Some(rest) = text.strip_prefix('[') {
        let (inner, after) = rest.split_once(']').ok_or(Error::MissingClosingBracket)?;
        if inner.is_empty() {
            return Err(Error::EmptyBrackets);
        }
        let port = if after.is_empty() {
            default_port
        } else if let Some(port_text) = after.strip_prefix(':') {
            parse_port(port_text)?
        } else {
            return Err(Error::TrailingAfterBracket);
        };
        return Ok((inner.to_string(), port));
    }

    match text.matches(':').count() {
        0 => Ok((text.to_string(), default_port)),
        1 => {
            let (host, port_text) = text.split_once(':').ok_or(Error::MissingHost)?;
            if host.is_empty() {
                return Err(Error::MissingHost);
            }
            Ok((host.to_string(), parse_port(port_text)?))
        }
        _ => {
            if text.parse::<Ipv6Addr>().is_ok() {
                Err(Error::UnbracketedIpv6)
            } else {
                Err(Error::MultipleColons)
            }
        }
    }
}

/// Parses a port: decimal digits only, value in `1..=65535`.
fn parse_port(text: &str) -> Result<u16> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidPort);
    }
    let mut value: u32 = 0;
    for b in text.bytes() {
        value = value * 10 + u32::from(b - b'0');
        if value > u32::from(u16::MAX) {
            return Err(Error::InvalidPort);
        }
    }
    match u16::try_from(value) {
        Ok(port) if port != 0 => Ok(port),
        _ => Err(Error::InvalidPort),
    }
}

/// Parses a server-vars string such as `\k1\v1\k2\v2`.
///
/// The input must start with a backslash. Fields are split on backslashes
/// and taken as key/value pairs in order. Keys and values may be empty.
/// An odd number of fields is an error. For duplicate keys the last one wins.
pub fn parse_server_vars(vars: &[u8]) -> Result<HashMap<Vec<u8>, Vec<u8>>> {
    let body = vars.strip_prefix(b"\\").ok_or(Error::BadVars)?;
    let fields: Vec<&[u8]> = body.split(|&b| b == b'\\').collect();
    if !fields.len().is_multiple_of(2) {
        return Err(Error::BadVars);
    }
    Ok(fields
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[k, v]| (k.to_vec(), v.to_vec()))
        .collect())
}

/// Parses a master-server reply into `(address, port)` pairs.
///
/// Layout: `H "getserversResponse"`, entries of 7 bytes (`\`, 4 IPv4 bytes,
/// 2 port bytes big-endian), then the terminator `\EOT\0\0\0`. Parsing
/// stops at the terminator; later bytes are ignored. The result keeps packet
/// order. A malformed entry or a missing terminator is an error; a partial
/// list is never returned. The terminator is checked before the entry
/// format, so a 7-byte block equal to the terminator ends the list.
pub fn parse_servers_response(packet: &[u8]) -> Result<Vec<(Ipv4Addr, u16)>> {
    let mut rest = strip_header(packet, b"getserversResponse")?;
    let mut servers = Vec::new();
    loop {
        if rest.is_empty() {
            return Err(Error::MissingTerminator);
        }
        let Some((entry, tail)) = rest.split_first_chunk::<7>() else {
            return Err(Error::BadServerEntry);
        };
        if *entry == SERVERS_TERMINATOR {
            return Ok(servers);
        }
        let [sep, a, b, c, d, hi, lo] = *entry;
        if sep != b'\\' {
            return Err(Error::BadServerEntry);
        }
        servers.push((Ipv4Addr::new(a, b, c, d), u16::from_be_bytes([hi, lo])));
        rest = tail;
    }
}
