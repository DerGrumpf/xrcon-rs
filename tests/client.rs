//! Tests for the client core (`xrcon::core::Session`), driven through the
//! `Harness` in `tests/common/mod.rs`: no socket, no clock.
//!
//! Each test feeds datagrams and time into the session and checks the
//! datagrams it sends and the events it reports.

mod common;

use common::{
    Event, GETCHALLENGE, Harness, Mode, PASSWORD, START_TIME, challenge_reply, rcon_reply,
};
use xrcon::packet::{rcon_nosecure_packet, rcon_secure_challenge_packet, rcon_secure_time_packet};

/// A challenge as a server would send it.
const CHALLENGE: [u8; 11] = *b"11111111111";
/// A second, different challenge.
const CHALLENGE_2: [u8; 11] = *b"abcdefghijk";

/// A harness in challenge mode.
fn challenge_harness() -> Harness {
    Harness::new(Mode::Challenge)
}

// ---------------------------------------------------------------------------
// Non-secure mode
// ---------------------------------------------------------------------------

/// Sends `status` in non-secure mode and checks that exactly one datagram,
/// the non-secure packet, is sent.
#[test]
fn nosecure_sends_one_packet() {
    let mut h = Harness::new(Mode::Nosecure);
    h.send(b"status");
    assert_eq!(
        h.take_sent(),
        vec![rcon_nosecure_packet(PASSWORD, b"status")]
    );
}

/// Non-secure mode starts no timer.
#[test]
fn nosecure_has_no_timer() {
    let mut h = Harness::new(Mode::Nosecure);
    h.send(b"status");
    assert_eq!(h.session.poll_timeout(), None);
}

// ---------------------------------------------------------------------------
// Time-secure mode
// ---------------------------------------------------------------------------

/// Sends `status` at time 100.0 with no time difference and checks that the
/// datagram is the time-secure packet for timestamp 100.0.
#[test]
fn time_sends_packet_with_current_time() {
    let mut h = Harness::new(Mode::Time);
    h.send(b"status");
    assert_eq!(
        h.take_sent(),
        vec![rcon_secure_time_packet(PASSWORD, b"status", START_TIME)]
    );
}

/// The timestamp is the time of the `send` call: after advancing the clock
/// by 5 seconds, the packet carries 105.0.
#[test]
fn time_uses_time_at_send() {
    let mut h = Harness::new(Mode::Time);
    h.advance(5.0);
    h.send(b"status");
    assert_eq!(
        h.take_sent(),
        vec![rcon_secure_time_packet(
            PASSWORD,
            b"status",
            START_TIME + 5.0
        )]
    );
}

/// A time difference of +7 seconds is added to the timestamp (107.0).
#[test]
fn time_positive_time_diff_is_added() {
    let mut h = Harness::new(Mode::Time);
    h.session.set_time_diff(7);
    h.send(b"status");
    assert_eq!(
        h.take_sent(),
        vec![rcon_secure_time_packet(
            PASSWORD,
            b"status",
            START_TIME + 7.0
        )]
    );
}

/// A negative time difference of -30 seconds is subtracted (70.0).
#[test]
fn time_negative_time_diff_is_subtracted() {
    let mut h = Harness::new(Mode::Time);
    h.session.set_time_diff(-30);
    h.send(b"status");
    assert_eq!(
        h.take_sent(),
        vec![rcon_secure_time_packet(
            PASSWORD,
            b"status",
            START_TIME - 30.0
        )]
    );
}

/// Time-secure mode starts no timer.
#[test]
fn time_has_no_timer() {
    let mut h = Harness::new(Mode::Time);
    h.send(b"status");
    assert_eq!(h.session.poll_timeout(), None);
}

// ---------------------------------------------------------------------------
// Challenge-secure mode: getting the challenge
// ---------------------------------------------------------------------------

/// Sending a command in challenge mode first sends only the `getchallenge`
/// request; the rcon packet waits for the challenge.
#[test]
fn challenge_send_requests_challenge_first() {
    let mut h = challenge_harness();
    h.send(b"status");
    assert_eq!(h.take_sent(), vec![GETCHALLENGE.to_vec()]);
}

/// While waiting for the challenge, the timer is set 0.5 seconds ahead
/// (the default challenge timeout).
#[test]
fn challenge_timer_default_is_half_a_second() {
    let mut h = challenge_harness();
    h.send(b"status");
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + 0.5));
}

/// When the challenge arrives, the session sends the challenge-secure packet
/// for the original command and that challenge.
#[test]
fn challenge_reply_triggers_rcon_packet() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.take_sent();
    h.recv(&challenge_reply(&CHALLENGE));
    assert_eq!(
        h.take_sent(),
        vec![rcon_secure_challenge_packet(
            PASSWORD, &CHALLENGE, b"status"
        )]
    );
}

/// After the rcon packet is sent, no timer is left running and no event was
/// reported.
#[test]
fn challenge_reply_clears_timer() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.recv(&challenge_reply(&CHALLENGE));
    assert_eq!(h.session.poll_timeout(), None);
    assert_eq!(h.take_events(), vec![]);
}

/// A datagram with the wrong header is ignored while waiting: nothing is
/// sent, no event is reported, and the timer keeps its deadline.
#[test]
fn challenge_bad_datagram_is_ignored() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.take_sent();
    h.recv(b"\xff\xff\xff\xffBAD PACKET");
    assert_eq!(h.take_sent(), Vec::<Vec<u8>>::new());
    assert_eq!(h.take_events(), vec![]);
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + 0.5));
}

/// A challenge reply with only 10 challenge bytes does not parse and is
/// ignored: no rcon packet is sent.
#[test]
fn challenge_short_challenge_is_ignored() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.take_sent();
    h.recv(&[common::HEADER, b"challenge ", b"abcdefghij"].concat());
    assert_eq!(h.take_sent(), Vec::<Vec<u8>>::new());
}

/// After a bad datagram, a good challenge reply still works.
#[test]
fn challenge_good_reply_after_bad_datagram() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.take_sent();
    h.recv(b"\xff\xff\xff\xffBAD PACKET");
    h.recv(&challenge_reply(&CHALLENGE));
    assert_eq!(
        h.take_sent(),
        vec![rcon_secure_challenge_packet(
            PASSWORD, &CHALLENGE, b"status"
        )]
    );
}

/// An rcon reply arriving while waiting for the challenge is reported as a
/// response and does not stop the wait.
#[test]
fn challenge_rcon_reply_while_waiting_is_a_response() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.take_sent();
    h.recv(&rcon_reply(b"late output"));
    assert_eq!(
        h.take_events(),
        vec![Event::Response(b"late output".to_vec())]
    );
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + 0.5));
}

/// Every command in challenge mode fetches a new challenge: a second send
/// after the first finished sends `getchallenge` again.
#[test]
fn challenge_each_send_fetches_a_new_challenge() {
    let mut h = challenge_harness();
    h.send(b"first");
    h.recv(&challenge_reply(&CHALLENGE));
    h.take_sent();
    h.send(b"second");
    assert_eq!(h.take_sent(), vec![GETCHALLENGE.to_vec()]);
    h.recv(&challenge_reply(&CHALLENGE_2));
    assert_eq!(
        h.take_sent(),
        vec![rcon_secure_challenge_packet(
            PASSWORD,
            &CHALLENGE_2,
            b"second"
        )]
    );
}

// ---------------------------------------------------------------------------
// Challenge-secure mode: retries and failure
// ---------------------------------------------------------------------------

/// Before the 0.5 s deadline, a timeout call does nothing.
#[test]
fn challenge_no_resend_before_deadline() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.take_sent();
    h.advance(0.4);
    assert_eq!(h.take_sent(), Vec::<Vec<u8>>::new());
    assert_eq!(h.take_events(), vec![]);
}

/// At the deadline the session sends `getchallenge` again and sets the timer
/// another 0.5 s ahead.
#[test]
fn challenge_resends_at_deadline() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.take_sent();
    h.advance(0.5);
    assert_eq!(h.take_sent(), vec![GETCHALLENGE.to_vec()]);
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + 1.0));
}

/// A challenge arriving after one retry still completes the command.
#[test]
fn challenge_reply_after_one_retry() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.advance(0.5);
    h.take_sent();
    h.recv(&challenge_reply(&CHALLENGE));
    assert_eq!(
        h.take_sent(),
        vec![rcon_secure_challenge_packet(
            PASSWORD, &CHALLENGE, b"status"
        )]
    );
    assert_eq!(h.session.poll_timeout(), None);
}

/// With no reply at all, `getchallenge` is sent 3 times in total (the first
/// send and two retries), and no rcon packet is ever sent.
#[test]
fn challenge_three_tries_in_total() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.advance(0.5);
    h.advance(0.5);
    h.advance(0.5);
    assert_eq!(h.take_sent(), vec![GETCHALLENGE.to_vec(); 3]);
}

/// After the third timeout the session reports `ChallengeFailed` once,
/// and no timer is left running. Before that, no event is reported.
#[test]
fn challenge_fails_after_third_timeout() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.advance(0.5);
    h.advance(0.5);
    assert_eq!(h.take_events(), vec![]);
    h.advance(0.5);
    assert_eq!(h.take_events(), vec![Event::ChallengeFailed]);
    assert_eq!(h.session.poll_timeout(), None);
}

/// After the failure, further time passing sends nothing and reports nothing.
#[test]
fn challenge_failure_is_final() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.advance(0.5);
    h.advance(0.5);
    h.advance(0.5);
    h.take_sent();
    h.take_events();
    h.advance(10.0);
    assert_eq!(h.take_sent(), Vec::<Vec<u8>>::new());
    assert_eq!(h.take_events(), vec![]);
}

/// A challenge reply arriving after the failure does not send the rcon
/// packet: that command is abandoned.
#[test]
fn challenge_late_reply_after_failure_is_ignored() {
    let mut h = challenge_harness();
    h.send(b"status");
    h.advance(0.5);
    h.advance(0.5);
    h.advance(0.5);
    h.take_sent();
    h.recv(&challenge_reply(&CHALLENGE));
    assert_eq!(h.take_sent(), Vec::<Vec<u8>>::new());
}

/// A new command after a failure starts fresh: it sends `getchallenge` and
/// gets the full number of tries again.
#[test]
fn challenge_new_send_after_failure_starts_fresh() {
    let mut h = challenge_harness();
    h.send(b"first");
    h.advance(0.5);
    h.advance(0.5);
    h.advance(0.5);
    h.take_sent();
    h.take_events();
    h.send(b"second");
    h.advance(0.5);
    h.advance(0.5);
    assert_eq!(h.take_sent(), vec![GETCHALLENGE.to_vec(); 3]);
    assert_eq!(h.take_events(), vec![]);
    h.advance(0.5);
    assert_eq!(h.take_events(), vec![Event::ChallengeFailed]);
}

/// With the retry count set to 1, `getchallenge` is sent once and the
/// command fails at the first deadline.
#[test]
fn challenge_retries_are_configurable() {
    let mut h = challenge_harness();
    h.session.set_challenge_retries(1);
    h.send(b"status");
    h.advance(0.5);
    assert_eq!(h.take_sent(), vec![GETCHALLENGE.to_vec()]);
    assert_eq!(h.take_events(), vec![Event::ChallengeFailed]);
}

/// With the timeout set to 2 seconds, the timer is 2 seconds ahead and no
/// retry happens before then.
#[test]
fn challenge_timeout_is_configurable() {
    let mut h = challenge_harness();
    h.session.set_challenge_timeout(2.0);
    h.send(b"status");
    assert_eq!(h.session.poll_timeout(), Some(START_TIME + 2.0));
    h.take_sent();
    h.advance(1.9);
    assert_eq!(h.take_sent(), Vec::<Vec<u8>>::new());
    h.advance(0.1);
    assert_eq!(h.take_sent(), vec![GETCHALLENGE.to_vec()]);
}

// ---------------------------------------------------------------------------
// Receiving rcon replies
// ---------------------------------------------------------------------------

/// One rcon reply becomes one `Response` event carrying its body.
#[test]
fn response_is_reported() {
    let mut h = Harness::new(Mode::Nosecure);
    h.recv(&rcon_reply(b"Test"));
    assert_eq!(h.take_events(), vec![Event::Response(b"Test".to_vec())]);
}

/// Several replies become several events, in the order received.
#[test]
fn responses_keep_order() {
    let mut h = Harness::new(Mode::Nosecure);
    h.recv(&rcon_reply(b"1"));
    h.recv(&rcon_reply(b"2"));
    h.recv(&rcon_reply(b"3"));
    assert_eq!(
        h.take_events(),
        vec![
            Event::Response(b"1".to_vec()),
            Event::Response(b"2".to_vec()),
            Event::Response(b"3".to_vec()),
        ]
    );
}

/// A reply with an empty body is reported as a response with an empty body.
#[test]
fn empty_response_is_reported() {
    let mut h = Harness::new(Mode::Nosecure);
    h.recv(&rcon_reply(b""));
    assert_eq!(h.take_events(), vec![Event::Response(vec![])]);
}

/// Replies are reported in every mode.
#[test]
fn response_is_reported_in_every_mode() {
    for mode in [Mode::Nosecure, Mode::Time, Mode::Challenge] {
        let mut h = Harness::new(mode);
        h.recv(&rcon_reply(b"Test"));
        assert_eq!(h.take_events(), vec![Event::Response(b"Test".to_vec())]);
    }
}

/// A datagram that is not an rcon reply produces no event.
#[test]
fn non_rcon_datagram_is_ignored() {
    let mut h = Harness::new(Mode::Nosecure);
    h.recv(b"bad packet");
    h.recv(b"\xff\xff\xff\xffstatusResponse\n\\gamename\\Xonotic\n");
    h.recv(b"");
    assert_eq!(h.take_events(), vec![]);
    assert_eq!(h.take_sent(), Vec::<Vec<u8>>::new());
}

// ---------------------------------------------------------------------------
// Changing settings on a live session
// ---------------------------------------------------------------------------

/// Changing the mode between commands changes the packet type of the next
/// command, as in the reference integration tests (non-secure, then time).
#[test]
fn mode_can_change_between_commands() {
    let mut h = Harness::new(Mode::Nosecure);
    h.send(b"status");
    h.session.set_mode(Mode::Time);
    h.send(b"status");
    assert_eq!(
        h.take_sent(),
        vec![
            rcon_nosecure_packet(PASSWORD, b"status"),
            rcon_secure_time_packet(PASSWORD, b"status", START_TIME),
        ]
    );
}

/// Changing the password applies to the next command.
#[test]
fn password_can_change_between_commands() {
    let mut h = Harness::new(Mode::Nosecure);
    h.session.set_password(b"other".to_vec());
    h.send(b"status");
    assert_eq!(
        h.take_sent(),
        vec![rcon_nosecure_packet(b"other", b"status")]
    );
}
