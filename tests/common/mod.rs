//! Shared helpers for the client tests (`tests/client.rs`, `tests/ping.rs`).
//!
//! Each test file that needs these declares `mod common;`.
//!
//! # The core API these helpers drive (`xrcon::core`)
//!
//! The core is sans-IO: it never touches a socket or a clock. The caller
//! passes every incoming datagram and the current time in, and collects what
//! the core wants to send and report.
//!
//! - `Session::new(password: impl Into<Vec<u8>>, mode: Mode) -> Session`
//! - `Session::set_time_diff(&mut self, seconds: i64)`
//! - `Session::set_challenge_timeout(&mut self, seconds: f64)`
//! - `Session::set_challenge_retries(&mut self, tries: u32)`
//! - `Session::send(&mut self, command: &[u8], now: f64)`: start sending an
//!   rcon command.
//! - `Session::handle_datagram(&mut self, datagram: &[u8], now: f64)`: feed
//!   one received datagram.
//! - `Session::handle_timeout(&mut self, now: f64)`: tell the core that time
//!   has passed.
//! - `Session::poll_transmit(&mut self) -> Option<Vec<u8>>`: the next
//!   datagram to send, if any.
//! - `Session::poll_event(&mut self) -> Option<Event>`: the next event, if
//!   any.
//! - `Session::poll_timeout(&self) -> Option<f64>`: when `handle_timeout`
//!   must be called next, if a timer is running.
//! - `Mode`: `Nosecure`, `Time`, `Challenge`.
//! - `Event` (must implement `Debug` and `PartialEq`):
//!   - `Event::Response(Vec<u8>)`: the body of one received rcon reply.
//!   - `Event::ChallengeFailed`: no challenge arrived after all retries.
//! - `Session::set_mode(&mut self, mode: Mode)`
//! - `Session::set_password(&mut self, password: impl Into<Vec<u8>>)`
//!
//! Time is a number of seconds as `f64`. The same value serves as the Unix
//! timestamp for time-secure packets and as the clock for timeouts.

#![allow(dead_code)] // every test file uses only a part of this module

pub use xrcon::core::{Event, Mode, Session};

/// Every DarkPlaces out-of-band packet starts with four 0xFF bytes.
pub const HEADER: &[u8] = b"\xff\xff\xff\xff";

/// The datagram a client sends to ask for a challenge.
pub const GETCHALLENGE: &[u8] = b"\xff\xff\xff\xffgetchallenge";

/// The rcon password used by every test unless it says otherwise.
pub const PASSWORD: &[u8] = b"passw";

/// The time at which a test starts, in seconds.
pub const START_TIME: f64 = 100.0;

/// Builds a challenge reply as a server sends it: header, `challenge `, the
/// 11-byte challenge, and a NUL terminator.
pub fn challenge_reply(challenge: &[u8; 11]) -> Vec<u8> {
    [HEADER, b"challenge ", challenge.as_slice(), b"\x00"].concat()
}

/// Builds an rcon reply as a server sends it: header, `n`, and the body.
pub fn rcon_reply(body: &[u8]) -> Vec<u8> {
    [HEADER, b"n", body].concat()
}

/// Drives a [`Session`] the way a real client would, without any socket or
/// clock, and records everything the session sends and reports.
///
/// The harness keeps its own current time (`now`). Every call passes that
/// time to the session, then drains the session's outgoing datagrams into
/// `sent` and its events into `events`, in the order they were produced.
pub struct Harness {
    /// The session under test.
    pub session: Session,
    /// The harness clock in seconds.
    pub now: f64,
    sent: Vec<Vec<u8>>,
    events: Vec<Event>,
}

impl Harness {
    /// A harness with a new session using [`PASSWORD`] and the given mode,
    /// starting at [`START_TIME`].
    pub fn new(mode: Mode) -> Self {
        Harness {
            session: Session::new(PASSWORD, mode),
            now: START_TIME,
            sent: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Asks the session to send an rcon command at the current time.
    pub fn send(&mut self, command: &[u8]) {
        self.session.send(command, self.now);
        self.drain();
    }

    /// Feeds one received datagram to the session at the current time.
    pub fn recv(&mut self, datagram: &[u8]) {
        self.session.handle_datagram(datagram, self.now);
        self.drain();
    }

    /// Moves the clock forward by `seconds` and tells the session that time
    /// has passed.
    pub fn advance(&mut self, seconds: f64) {
        self.now += seconds;
        self.session.handle_timeout(self.now);
        self.drain();
    }

    /// Returns all datagrams sent since the last call, oldest first.
    pub fn take_sent(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.sent)
    }

    /// Returns all events reported since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Moves everything the session has queued into the harness's records.
    fn drain(&mut self) {
        while let Some(datagram) = self.session.poll_transmit() {
            self.sent.push(datagram);
        }
        while let Some(event) = self.session.poll_event() {
            self.events.push(event);
        }
    }

    /// Starts a ping at the current time.
    pub fn ping(&mut self, kind: xrcon::core::PingKind, timeout: f64) {
        self.session.ping(kind, timeout, self.now);
        self.drain();
    }

    /// Starts a status query at the current time.
    pub fn getstatus(&mut self, timeout: f64) {
        self.session.getstatus(timeout, self.now);
        self.drain();
    }
}
