//! Building outgoing rcon packets.
//!
//! Password and command are arbitrary bytes and are copied unchanged.
//! No size limit is enforced here; the caller must keep packets within
//! [`MAX_PACKET_SIZE`].

use hmac::{Hmac, KeyInit, Mac};
use md4::Md4;

/// Four `0xFF` bytes that start every out-of-band packet.
pub const HEADER: [u8; 4] = [0xFF; 4];

/// Maximum size of a packet on the wire, in bytes.
pub const MAX_PACKET_SIZE: usize = 1400;

/// Computes HMAC-MD4 over the concatenation of `parts`.
fn hmac_md4(key: &[u8], parts: &[&[u8]]) -> Vec<u8> {
    // HMAC accepts keys of any length, so this cannot fail.
    let mut mac =
        <Hmac<Md4> as KeyInit>::new_from_slice(key).expect("HMAC accepts keys of any length");
    for part in parts {
        mac.update(part);
    }
    mac.finalize().into_bytes()[..].to_vec()
}

/// Builds a non-secure rcon packet: `H "rcon " password " " command`.
pub fn rcon_nosecure_packet(password: &[u8], command: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER.len() + 6 + password.len() + command.len());
    out.extend_from_slice(&HEADER);
    out.extend_from_slice(b"rcon ");
    out.extend_from_slice(password);
    out.push(b' ');
    out.extend_from_slice(command);
    out
}

/// Builds a time-secure rcon packet:
/// `H "srcon HMAC-MD4 TIME " MAC " " T " " command`.
///
/// `time` is the final timestamp in seconds (time difference already added).
/// `T` is printed with exactly 6 decimals. `MAC` is the 16 raw bytes of
/// HMAC-MD4 with `password` as key over `T " " command`.
pub fn rcon_secure_time_packet(password: &[u8], command: &[u8], time: f64) -> Vec<u8> {
    let stamp = format!("{time:.6}");
    let mac = hmac_md4(password, &[stamp.as_bytes(), b" ", command]);
    let mut out =
        Vec::with_capacity(HEADER.len() + 20 + mac.len() + stamp.len() + command.len() + 2);
    out.extend_from_slice(&HEADER);
    out.extend_from_slice(b"srcon HMAC-MD4 TIME ");
    out.extend_from_slice(&mac);
    out.push(b' ');
    out.extend_from_slice(stamp.as_bytes());
    out.push(b' ');
    out.extend_from_slice(command);
    out
}

/// Builds a challenge-secure rcon packet:
/// `H "srcon HMAC-MD4 CHALLENGE " MAC " " challenge " " command`.
///
/// `MAC` is the 16 raw bytes of HMAC-MD4 with `password` as key over
/// `challenge " " command`.
pub fn rcon_secure_challenge_packet(password: &[u8], challenge: &[u8], command: &[u8]) -> Vec<u8> {
    let mac = hmac_md4(password, &[challenge, b" ", command]);
    let mut out =
        Vec::with_capacity(HEADER.len() + 26 + mac.len() + challenge.len() + command.len() + 2);
    out.extend_from_slice(&HEADER);
    out.extend_from_slice(b"srcon HMAC-MD4 CHALLENGE ");
    out.extend_from_slice(&mac);
    out.push(b' ');
    out.extend_from_slice(challenge);
    out.push(b' ');
    out.extend_from_slice(command);
    out
}
