//! Tests for pings and status queries on the client core
//! (`xrcon::core::Session`), driven through the `Harness` in
//! `tests/common/mod.rs`.
//!
//! # API added to the core by these tests
//!
//! - `Session::ping(&mut self, kind: PingKind, timeout: f64, now: f64)`: send
//!   a ping and wait for the matching pong for `timeout` seconds.
//! - `Session::getstatus(&mut self, timeout: f64, now: f64)`: send a status
//!   request and wait for the reply for `timeout` seconds.
//! - `PingKind`: `Q2` (ping with header, answer `ack`) and `Q3` (ping without
//!   header, answer `disconnect`).
//! - New `Event` variants:
//!   - `Event::Pong { kind: PingKind, rtt: f64 }`: the pong arrived after
//!     `rtt` seconds.
//!   - `Event::PingTimeout(PingKind)`: no matching pong within the timeout.
//!   - `Event::Status { vars, players }`: a parsed status reply, with `vars`
//!     as in `parse_server_vars` and `players` a `Vec<Player>`.
//!   - `Event::StatusInvalid`: a reply with the status header whose content
//!     does not parse.
//!   - `Event::StatusTimeout`: no status reply within the timeout.
//!
//! Time values in these tests are exact binary fractions (0.25, 0.5), so the
//! round-trip times compare exactly.

mod common;

use common::{Event, HEADER, Harness, Mode, START_TIME, rcon_reply};
use xrcon::core::PingKind;

/// Default ping timeout of the Python client (`ping2(timeout=1)`).
const PING_TIMEOUT: f64 = 1.0;
/// Default status timeout of the Python client (`CHALLENGE_TIMEOUT = 3`).
const STATUS_TIMEOUT: f64 = 3.0;

const PING_Q2: &[u8] = b"\xff\xff\xff\xffping";
const PONG_Q2: &[u8] = b"\xff\xff\xff\xffack";
const PING_Q3: &[u8] = b"ping";
const PONG_Q3: &[u8] = b"\xff\xff\xff\xffdisconnect";
const GETSTATUS: &[u8] = b"\xff\xff\xff\xffgetstatus";

/// A valid status reply: two server variables and one player.
fn status_reply() -> Vec<u8> {
    [
        HEADER,
        b"statusResponse\n\\gamename\\Xonotic\\mapname\\lostspace2\n4 5 \"Player\"\n",
    ]
    .concat()
}

/// A harness (non-secure mode; the mode does not matter for these tests).
fn harness() -> Harness {
    Harness::new(Mode::Nosecure)
}

/// Extracts `(name, rtt)` checks are done directly in each test; this only
/// starts a Q2 ping with the default timeout and drops the sent datagram.
fn started_q2() -> Harness {
    let mut h = harness();
    h.ping(PingKind::Q2, PING_TIMEOUT);
    h.take_sent();
    h
}

// ---------------------------------------------------------------------------
// Q2 ping: `header + "ping"`, answer exactly `header + "ack"`
// ---------------------------------------------------------------------------

/// Starting a Q2 ping sends exactly one datagram: `header + ping`.
#[test]
fn q2_ping_sends_ping_packet() {
    let mut h = harness();
    h.ping(PingKind::Q2, PING_TIMEOUT);
    assert_eq!(h.take_sent(), vec![PING_Q2.to_vec()]);
}

/// The timer is set to the start time plus the timeout.
#[test]
fn q2_ping_sets_timer() {
    let h = started_q2();
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + PING_TIMEOUT));
}

/// The pong `header + ack` arriving 0.25 s after the ping gives a `Pong`
/// event with a round-trip time of 0.25 s.
#[test]
fn q2_pong_reports_round_trip_time() {
    let mut h = started_q2();
    h.now += 0.25;
    h.recv(PONG_Q2);
    assert_eq!(
        h.take_events(),
        vec![Event::Pong {
            kind: PingKind::Q2,
            rtt: 0.25
        }]
    );
}

/// After the pong, no timer is left running.
#[test]
fn q2_pong_clears_timer() {
    let mut h = started_q2();
    h.recv(PONG_Q2);
    assert_eq!(h.session.poll_timeout(), None);
}

/// A second pong after the first is ignored: the ping is reported once.
#[test]
fn q2_second_pong_is_ignored() {
    let mut h = started_q2();
    h.recv(PONG_Q2);
    h.take_events();
    h.recv(PONG_Q2);
    assert_eq!(h.take_events(), vec![]);
}

/// A pong with extra bytes after `ack` is not a pong: the reference compares
/// the whole datagram. The ping keeps waiting.
#[test]
fn q2_pong_with_trailing_bytes_is_ignored() {
    let mut h = started_q2();
    h.recv(b"\xff\xff\xff\xffack extra");
    assert_eq!(h.take_events(), vec![]);
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + PING_TIMEOUT));
}

/// The Q3 pong (`disconnect`) does not answer a Q2 ping.
#[test]
fn q2_ping_ignores_q3_pong() {
    let mut h = started_q2();
    h.recv(PONG_Q3);
    assert_eq!(h.take_events(), vec![]);
}

/// An unrelated datagram is ignored and the ping keeps waiting.
#[test]
fn q2_ping_ignores_other_datagrams() {
    let mut h = started_q2();
    h.recv(b"bad packet");
    h.recv(b"");
    assert_eq!(h.take_events(), vec![]);
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + PING_TIMEOUT));
}

/// A pong arriving after a good pong-shaped datagram that did not match is
/// still accepted: ignored datagrams do not end the ping.
#[test]
fn q2_pong_after_ignored_datagram() {
    let mut h = started_q2();
    h.recv(b"bad packet");
    h.now += 0.5;
    h.recv(PONG_Q2);
    assert_eq!(
        h.take_events(),
        vec![Event::Pong {
            kind: PingKind::Q2,
            rtt: 0.5
        }]
    );
}

/// Before the deadline a timeout call reports nothing.
#[test]
fn q2_no_timeout_before_deadline() {
    let mut h = started_q2();
    h.advance(0.75);
    assert_eq!(h.take_events(), vec![]);
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + PING_TIMEOUT));
}

/// At the deadline the ping ends with a `PingTimeout` event and no timer is
/// left. The reference loop runs while the time is below the deadline, so
/// the deadline itself counts as timed out.
#[test]
fn q2_timeout_at_deadline() {
    let mut h = started_q2();
    h.advance(PING_TIMEOUT);
    assert_eq!(h.take_events(), vec![Event::PingTimeout(PingKind::Q2)]);
    assert_eq!(h.session.poll_timeout(), None);
}

/// The timeout is reported once: more time passing reports nothing more.
#[test]
fn q2_timeout_is_reported_once() {
    let mut h = started_q2();
    h.advance(PING_TIMEOUT);
    h.take_events();
    h.advance(10.0);
    assert_eq!(h.take_events(), vec![]);
}

/// A pong after the timeout is ignored.
#[test]
fn q2_late_pong_is_ignored() {
    let mut h = started_q2();
    h.advance(PING_TIMEOUT);
    h.take_events();
    h.recv(PONG_Q2);
    assert_eq!(h.take_events(), vec![]);
}

/// A pong when no ping was started is ignored.
#[test]
fn q2_unsolicited_pong_is_ignored() {
    let mut h = harness();
    h.recv(PONG_Q2);
    assert_eq!(h.take_events(), vec![]);
}

/// The timeout is the value passed to `ping`: with 2.5s the timer is 2.5s ahead.
#[test]
fn q2_custom_timeout() {
    let mut h = harness();
    h.ping(PingKind::Q2, 2.5);
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + 2.5));
}

/// An rcon reply arriving during a ping is reported as a response and does
/// not end the ping.
#[test]
fn q2_ping_keeps_waiting_when_rcon_reply_arrives() {
    let mut h = started_q2();
    h.recv(&rcon_reply(b"output"));
    assert_eq!(h.take_events(), vec![Event::Response(b"output".to_vec())]);
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + PING_TIMEOUT));
}

// ---------------------------------------------------------------------------
// Q3 ping: `ping` without header, answer exactly `header + "disconnect"`
// ---------------------------------------------------------------------------

/// Starting a Q3 ping sends the bare text `ping`, with no 0xFF header.
#[test]
fn q3_ping_sends_bare_ping() {
    let mut h = harness();
    h.ping(PingKind::Q3, PING_TIMEOUT);
    assert_eq!(h.take_sent(), vec![PING_Q3.to_vec()]);
}

/// The pong `header + disconnect` arriving 0.5 s later gives a `Pong` event
/// with a round-trip time of 0.5 s.
#[test]
fn q3_pong_reports_round_trip_time() {
    let mut h = harness();
    h.ping(PingKind::Q3, PING_TIMEOUT);
    h.take_sent();
    h.now += 0.5;
    h.recv(PONG_Q3);
    assert_eq!(
        h.take_events(),
        vec![Event::Pong {
            kind: PingKind::Q3,
            rtt: 0.5
        }]
    );
}

/// The Q2 pong (`ack`) does not answer a Q3 ping.
#[test]
fn q3_ping_ignores_q2_pong() {
    let mut h = harness();
    h.ping(PingKind::Q3, PING_TIMEOUT);
    h.take_sent();
    h.recv(PONG_Q2);
    assert_eq!(h.take_events(), vec![]);
}

/// A pong with extra bytes after `disconnect` is not a pong.
#[test]
fn q3_pong_with_trailing_bytes_is_ignored() {
    let mut h = harness();
    h.ping(PingKind::Q3, PING_TIMEOUT);
    h.take_sent();
    h.recv(b"\xff\xff\xff\xffdisconnect extra");
    assert_eq!(h.take_events(), vec![]);
}

/// A Q3 ping times out at its deadline with `PingTimeout(Q3)`.
#[test]
fn q3_timeout_at_deadline() {
    let mut h = harness();
    h.ping(PingKind::Q3, PING_TIMEOUT);
    h.take_sent();
    h.advance(PING_TIMEOUT);
    assert_eq!(h.take_events(), vec![Event::PingTimeout(PingKind::Q3)]);
}

// ---------------------------------------------------------------------------
// Status query: `header + "getstatus"`, answer `header + "statusResponse\n..."`
// ---------------------------------------------------------------------------

/// Starting a status query sends exactly one datagram: `header + getstatus`.
#[test]
fn status_sends_getstatus() {
    let mut h = harness();
    h.getstatus(STATUS_TIMEOUT);
    assert_eq!(h.take_sent(), vec![GETSTATUS.to_vec()]);
}

/// The timer is set to the start time plus the timeout.
#[test]
fn status_sets_timer() {
    let mut h = harness();
    h.getstatus(STATUS_TIMEOUT);
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + STATUS_TIMEOUT));
}

/// A valid status reply gives a `Status` event with the parsed variables
/// and players, and clears the timer.
#[test]
fn status_reply_is_parsed() {
    let mut h = harness();
    h.getstatus(STATUS_TIMEOUT);
    h.take_sent();
    h.recv(&status_reply());
    let events = h.take_events();
    assert_eq!(events.len(), 1);
    let Event::Status { vars, players } = &events[0] else {
        panic!("expected a Status event, got {:?}", events[0]);
    };
    assert_eq!(vars.len(), 2);
    assert_eq!(vars[&b"gamename"[..]], b"Xonotic");
    assert_eq!(vars[&b"mapname"[..]], b"lostspace2");
    assert_eq!(players.len(), 1);
    assert_eq!((players[0].frags, players[0].ping), (4, 5));
    assert_eq!(players[0].name, b"Player");
    assert_eq!(h.session.poll_timeout(), None);
}

/// A datagram that is not a status reply is ignored and the query keeps
/// waiting.
#[test]
fn status_ignores_other_datagrams() {
    let mut h = harness();
    h.getstatus(STATUS_TIMEOUT);
    h.take_sent();
    h.recv(b"bad packet");
    h.recv(b"\xff\xff\xff\xffack");
    assert_eq!(h.take_events(), vec![]);
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + STATUS_TIMEOUT));
}

/// A status reply after ignored datagrams is still accepted.
#[test]
fn status_reply_after_ignored_datagram() {
    let mut h = harness();
    h.getstatus(STATUS_TIMEOUT);
    h.take_sent();
    h.recv(b"bad packet");
    h.recv(&status_reply());
    assert!(matches!(h.take_events().as_slice(), [Event::Status { .. }]));
}

/// A reply with the status header whose content does not parse gives
/// `StatusInvalid`, and the query ends (no timer left).
#[test]
fn status_invalid_content_is_reported() {
    let mut h = harness();
    h.getstatus(STATUS_TIMEOUT);
    h.take_sent();
    h.recv(&[HEADER, b"statusResponse\nnot server vars\n".as_slice()].concat());
    assert_eq!(h.take_events(), vec![Event::StatusInvalid]);
    assert_eq!(h.session.poll_timeout(), None);
}

/// A valid reply is reported once; a second reply is ignored.
#[test]
fn status_second_reply_is_ignored() {
    let mut h = harness();
    h.getstatus(STATUS_TIMEOUT);
    h.take_sent();
    h.recv(&status_reply());
    h.take_events();
    h.recv(&status_reply());
    assert_eq!(h.take_events(), vec![]);
}

/// Before the deadline a timeout call reports nothing.
#[test]
fn status_no_timeout_before_deadline() {
    let mut h = harness();
    h.getstatus(STATUS_TIMEOUT);
    h.take_sent();
    h.advance(2.75);
    assert_eq!(h.take_events(), vec![]);
}

/// At the deadline the query ends with `StatusTimeout` and no timer is left.
#[test]
fn status_timeout_at_deadline() {
    let mut h = harness();
    h.getstatus(STATUS_TIMEOUT);
    h.take_sent();
    h.advance(STATUS_TIMEOUT);
    assert_eq!(h.take_events(), vec![Event::StatusTimeout]);
    assert_eq!(h.session.poll_timeout(), None);
}

/// A status reply after the timeout is ignored.
#[test]
fn status_late_reply_is_ignored() {
    let mut h = harness();
    h.getstatus(STATUS_TIMEOUT);
    h.take_sent();
    h.advance(STATUS_TIMEOUT);
    h.take_events();
    h.recv(&status_reply());
    assert_eq!(h.take_events(), vec![]);
}

/// A status reply when no query was started is ignored.
#[test]
fn status_unsolicited_reply_is_ignored() {
    let mut h = harness();
    h.recv(&status_reply());
    assert_eq!(h.take_events(), vec![]);
}

/// The timeout is the value passed to `getstatus`:
/// with 0.5s the timer is 0.5s ahead.
#[test]
fn status_custom_timeout() {
    let mut h = harness();
    h.getstatus(0.5);
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + 0.5));
}
