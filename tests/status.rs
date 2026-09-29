//! Tests for parsing server status replies (`xrcon::parse`): the server
//! variables string, one player line, and a full `statusResponse` packet.
//!
//! API under test:
//! - `parse_server_vars(&[u8]) -> Result<HashMap<Vec<u8>, Vec<u8>>, _>`
//! - `Player::parse(&[u8]) -> Result<Player, _>` with fields `frags: i32`,
//!   `ping: i32`, `name: Vec<u8>`
//! - `parse_status_packet(&[u8]) -> Result<(HashMap<Vec<u8>, Vec<u8>>, Vec<Player>), _>`
//!
//! Where the expected behaviour comes from:
//! - **reference**: the vector or case appears in `xrcon/tests/library_test.py`.
//! - **reference code**: not in a reference test, but it is what the parsing
//!   code in `xrcon/utils.py` does (regular expression, splitting rules).
//! - **decision**: follows the project decision that only valid packets are
//!   accepted, applied to the status header as it was to the other headers.

use xrcon::parse::{Player, parse_server_vars, parse_status_packet};

/// Every DarkPlaces out-of-band packet starts with four 0xFF bytes.
const HEADER: &[u8] = b"\xff\xff\xff\xff";

/// Builds a status reply: header + `statusResponse\n` + `body`.
fn status_reply(body: &[u8]) -> Vec<u8> {
    [HEADER, b"statusResponse\n", body].concat()
}

/// 11 server variables and 7 players, with colour codes and UTF-8 bytes in the names.
fn reference_status_packet() -> Vec<u8> {
    [
        b"\xff\xff\xff\xffstatusResponse\n\\gamename\\Xonotic\\modname\\data\\gameversion\\700\\sv_maxclients\\14\\clients\\12\\".as_slice(),
        b"bots\\0\\mapname\\lostspace2\\hostname\\Xonotic 0.7.0 CTF Server\\protocol\\3\\qcstatus\\ctf:0.7.0:P315:S2:F6:MMinstaGib::score!!:caps!!:5:9:14:8\\d0_blind_id\\1 HSALTRUJf4ShAOk+oZDUOtijIquc73hCoO9tai62qRE=@Xon//KssdlzGkFKdnnN4sgg8H+koTbBn5JTi37BAW1Q=\n",
        b"57 72 \"^x555[^x0BF\xe5\x8a\x9b^x777] ^xBBBIn^1t^7eger\"\n319 116 \"^x777Akd^7\"\n-666 41 \"me\"\n14 53 \"^xF00\xc3\x89^7\"\n",
        b"130 66 \"icetea\"\n98 33 \"^x0c0*^x333r^x222e^x111i^7\"\n56 49 \"^x940S^x720Weed^x530^7\"\n",
    ]
    .concat()
}

// ---------------------------------------------------------------------------
// Server variables: \key1\value1\key2\value2...
// ---------------------------------------------------------------------------

/// Parses three key/value pairs and checks every value and the pair count.
#[test]
fn server_vars_reference_vector() {
    let vars =
        parse_server_vars(b"\\gamename\\somegame\\modname\\somemod\\gameversion\\100").unwrap();
    assert_eq!(vars.len(), 3);
    assert_eq!(vars[&b"gamename"[..]], b"somegame");
    assert_eq!(vars[&b"modname"[..]], b"somemod");
    assert_eq!(vars[&b"gameversion"[..]], b"100");
}

/// Text that does not start with a backslash is an error.
#[test]
fn server_vars_bad_data_is_rejected() {
    assert!(parse_server_vars(b"bad data").is_err());
}

/// An empty string is an error, because it does not start with a backslash.
#[test]
fn server_vars_empty_input_is_rejected() {
    assert!(parse_server_vars(b"").is_err());
}

/// A single pair returns a map with exactly one entry.
#[test]
fn server_vars_single_pair() {
    let vars = parse_server_vars(b"\\mapname\\lostspace2").unwrap();
    assert_eq!(vars.len(), 1);
    assert_eq!(vars[&b"mapname"[..]], b"lostspace2");
}

/// A value may be empty: `\a\\b\c` gives `a = ""` and `b = "c"`.
#[test]
fn server_vars_empty_value() {
    let vars = parse_server_vars(b"\\a\\\\b\\c").unwrap();
    assert_eq!(vars.len(), 2);
    assert_eq!(vars[&b"a"[..]], b"");
    assert_eq!(vars[&b"b"[..]], b"c");
}

/// Values may contain spaces and other punctuation and are returned
/// unchanged, as are non-ASCII bytes.
#[test]
fn server_vars_values_are_returned_verbatim() {
    let vars = parse_server_vars(b"\\hostname\\My Server: 0.7 [CTF] \xc3\xa4").unwrap();
    assert_eq!(
        vars[&b"hostname"[..]],
        &b"My Server: 0.7 [CTF] \xc3\xa4"[..]
    );
}

// ---------------------------------------------------------------------------
// One player line: <frags> <ping> "<name>"
// ---------------------------------------------------------------------------

/// Parses `4 5 "Player"` and checks frags, ping and name.
#[test]
fn player_reference_vector() {
    let p = Player::parse(b"4 5 \"Player\"").unwrap();
    assert_eq!(p.frags, 4);
    assert_eq!(p.ping, 5);
    assert_eq!(p.name, b"Player");
}

/// Frags may be negative (`-666`).
#[test]
fn player_negative_frags() {
    let p = Player::parse(b"-666 5 \"Player\"").unwrap();
    assert_eq!(p.frags, -666);
    assert_eq!(p.ping, 5);
}

/// A name without its closing quote is an error.
#[test]
fn player_missing_closing_quote_is_rejected() {
    assert!(Player::parse(b"666 5 \"Player").is_err());
}

/// Ping may be negative too, because the reference pattern allows a minus
/// sign in both numbers.
#[test]
fn player_negative_ping() {
    let p = Player::parse(b"4 -1 \"Player\"").unwrap();
    assert_eq!(p.frags, 4);
    assert_eq!(p.ping, -1);
}

/// An empty name (`""`) is valid and gives an empty name.
#[test]
fn player_empty_name() {
    let p = Player::parse(b"1 2 \"\"").unwrap();
    assert_eq!(p.name, b"");
}

/// A name may contain spaces, colour codes and non-ASCII bytes; it is
/// returned unchanged without the two enclosing quotes.
#[test]
fn player_name_is_returned_verbatim() {
    let p = Player::parse(b"1 2 \"^x555Some Name \xc3\x89^7\"").unwrap();
    assert_eq!(p.name, &b"^x555Some Name \xc3\x89^7"[..]);
}

/// A name may contain a quote character: only the last quote of the line
/// closes the name, so `"a"b"` has the name `a"b`.
#[test]
fn player_name_with_inner_quote() {
    let p = Player::parse(b"1 2 \"a\"b\"").unwrap();
    assert_eq!(p.name, &b"a\"b"[..]);
}

/// Text that is not a player line at all is an error.
#[test]
fn player_garbage_is_rejected() {
    assert!(Player::parse(b"bad data").is_err());
}

/// An empty line is an error.
#[test]
fn player_empty_line_is_rejected() {
    assert!(Player::parse(b"").is_err());
}

/// A non-numeric frags field is an error.
#[test]
fn player_non_numeric_frags_is_rejected() {
    assert!(Player::parse(b"x 5 \"Player\"").is_err());
}

/// A non-numeric ping field is an error.
#[test]
fn player_non_numeric_ping_is_rejected() {
    assert!(Player::parse(b"4 x \"Player\"").is_err());
}

/// A line without the ping field is an error.
#[test]
fn player_missing_ping_is_rejected() {
    assert!(Player::parse(b"4 \"Player\"").is_err());
}

/// A name that is not quoted is an error.
#[test]
fn player_unquoted_name_is_rejected() {
    assert!(Player::parse(b"4 5 Player").is_err());
}

/// Text after the closing quote is an error: the line must end with it.
#[test]
fn player_trailing_text_is_rejected() {
    assert!(Player::parse(b"4 5 \"Player\" extra").is_err());
}

/// Leading whitespace before the first number is an error: the pattern
/// starts at the beginning of the line.
#[test]
fn player_leading_space_is_rejected() {
    assert!(Player::parse(b" 4 5 \"Player\"").is_err());
}

// ---------------------------------------------------------------------------
// Full status packet: header + "statusResponse\n" + vars line + player lines
// ---------------------------------------------------------------------------

/// Parses the full reference packet and checks all 11 server variables.
#[test]
fn status_packet_reference_server_vars() {
    let (vars, _) = parse_status_packet(&reference_status_packet()).unwrap();
    let expected: &[(&[u8], &[u8])] = &[
        (b"gamename", b"Xonotic"),
        (b"modname", b"data"),
        (b"gameversion", b"700"),
        (b"sv_maxclients", b"14"),
        (b"clients", b"12"),
        (b"bots", b"0"),
        (b"mapname", b"lostspace2"),
        (b"hostname", b"Xonotic 0.7.0 CTF Server"),
        (b"protocol", b"3"),
        (b"qcstatus", b"ctf:0.7.0:P315:S2:F6:MMinstaGib::score!!:caps!!:5:9:14:8"),
        (
            b"d0_blind_id",
            b"1 HSALTRUJf4ShAOk+oZDUOtijIquc73hCoO9tai62qRE=@Xon//KssdlzGkFKdnnN4sgg8H+koTbBn5JTi37BAW1Q=",
        ),
    ];
    assert_eq!(vars.len(), expected.len());
    for (key, value) in expected {
        assert_eq!(
            vars[*key],
            *value,
            "server var {:?}",
            String::from_utf8_lossy(key)
        );
    }
}

/// Parses the full reference packet and checks that it holds 7 players and
/// that the third one is named `me`.
#[test]
fn status_packet_reference_players_count_and_third_name() {
    let (_, players) = parse_status_packet(&reference_status_packet()).unwrap();
    assert_eq!(players.len(), 7);
    assert_eq!(players[2].name, b"me");
}

/// Checks frags, ping and name of every player in the reference packet, in order.
#[test]
fn status_packet_reference_all_players() {
    let (_, players) = parse_status_packet(&reference_status_packet()).unwrap();
    let expected: &[(i32, i32, &[u8])] = &[
        (57, 72, b"^x555[^x0BF\xe5\x8a\x9b^x777] ^xBBBIn^1t^7eger"),
        (319, 116, b"^x777Akd^7"),
        (-666, 41, b"me"),
        (14, 53, b"^xF00\xc3\x89^7"),
        (130, 66, b"icetea"),
        (98, 33, b"^x0c0*^x333r^x222e^x111i^7"),
        (56, 49, b"^x940S^x720Weed^x530^7"),
    ];
    assert_eq!(players.len(), expected.len());
    for (player, (frags, ping, name)) in players.iter().zip(expected) {
        assert_eq!(
            (player.frags, player.ping, player.name.as_slice()),
            (*frags, *ping, *name)
        );
    }
}

/// A packet with server variables and no player lines is valid and returns
/// an empty player list.
#[test]
fn status_packet_without_players() {
    let (vars, players) = parse_status_packet(&status_reply(b"\\gamename\\Xonotic\n")).unwrap();
    assert_eq!(vars.len(), 1);
    assert_eq!(vars[&b"gamename"[..]], b"Xonotic");
    assert!(players.is_empty());
}

/// Random text repeated 40 times is an error.
#[test]
fn status_packet_garbage_is_rejected() {
    assert!(parse_status_packet(&b"BAD DATA".repeat(40)).is_err());
}

/// An empty packet is an error.
#[test]
fn status_packet_empty_is_rejected() {
    assert!(parse_status_packet(b"").is_err());
}

/// The header and `statusResponse\n` with nothing after it is an error,
/// because there is no server variables line.
#[test]
fn status_packet_header_only_is_rejected() {
    assert!(parse_status_packet(&status_reply(b"")).is_err());
}

/// A packet whose variables line does not start with a backslash is an error.
#[test]
fn status_packet_bad_server_vars_is_rejected() {
    assert!(parse_status_packet(&status_reply(b"gamename Xonotic\n")).is_err());
}

/// One malformed player line makes the whole packet an error, even when the
/// variables and the other players are valid.
#[test]
fn status_packet_bad_player_line_is_rejected() {
    let body = b"\\gamename\\Xonotic\n4 5 \"Good\"\nthis is not a player\n";
    assert!(parse_status_packet(&status_reply(body)).is_err());
}

/// A correct-length wrong header (`\xff\xff\xff\xffstatusRespons\n`, one
/// letter short) is an error even if the body is valid.
#[test]
fn status_packet_wrong_header_text_is_rejected() {
    let packet = [HEADER, b"statusRespons\n\\gamename\\Xonotic\n".as_slice()].concat();
    assert!(parse_status_packet(&packet).is_err());
}

/// A packet without the four 0xFF bytes is an error even if the rest is valid.
#[test]
fn status_packet_missing_ff_header_is_rejected() {
    assert!(parse_status_packet(b"statusResponse\n\\gamename\\Xonotic\n").is_err());
}

/// An rcon reply (`n` prefix) is not a status packet.
#[test]
fn status_packet_rejects_rcon_reply() {
    assert!(parse_status_packet(b"\xff\xff\xff\xffn\\gamename\\Xonotic\n").is_err());
}
