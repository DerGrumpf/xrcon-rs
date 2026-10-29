//! Sans-IO client core: no sockets, no clock. Time is passed in as seconds.
//!
//! The caller feeds datagrams and the current time in, and drains outgoing
//! datagrams, events and the pending deadline out.
//!
//! # Concurrent operations
//!
//! The session tracks one pending operation at a time (a challenge
//! handshake or a ping). Starting a new one (`ping` or `send` in
//! [`Mode::Challenge`]) silently replaces the
//! pending one: no event is emitted for the replaced operation, and its
//! deadline is dropped. `send` in [`Mode::Nosecure`] or [`Mode::Time`] does
//! not touch the pending operation. rcon replies are handled independently
//! of the pending operation.

use std::collections::VecDeque;

use crate::packet::{
    HEADER, rcon_nosecure_packet, rcon_secure_challenge_packet, rcon_secure_time_packet,
};
use crate::parse::{parse_challenge_response, parse_rcon_response};

/// How `send` authenticates rcon commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Plain `rcon <password> <command>` packets.
    Nosecure,
    /// Time-secure `srcon HMAC-MD4 TIME` packets.
    Time,
    /// Challenge-secure `srcon HMAC-MD4 CHALLENGE` packets.
    Challenge,
}

/// Which ping protocol variant to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PingKind {
    /// Quake 2 style: `H "ping"`, answered by `H "ack"`.
    Q2,
    /// Quake 3 style: `"ping"` without header, answered by `H "disconnect"`.
    Q3,
}

/// Something the session reports to the caller.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A valid rcon reply arrived; carries the body.
    Response(Vec<u8>),
    /// Every `getchallenge` try timed out; the command was abandoned.
    ChallengeFailed,
    /// The pong for a ping arrived.
    Pong {
        /// Kind of the ping that was answered.
        kind: PingKind,
        /// Time of the pong minus the start time of the ping, in seconds.
        rtt: f64,
    },
    /// A ping was not answered before its deadline.
    PingTimeout(PingKind),
}

/// Pending challenge handshake. Settings are captured when `send` is called.
struct ChallengeOp {
    command: Vec<u8>,
    password: Vec<u8>,
    timeout: f64,
    /// `getchallenge` sends still allowed after the ones already made.
    tries_left: u32,
    deadline: f64,
}

/// The single pending operation.
enum Op {
    Idle,
    Challenge(ChallengeOp),
    Ping {
        kind: PingKind,
        start: f64,
        deadline: f64,
    },
}

/// Sans-IO rcon client session.
///
/// See the [module documentation](self) for how concurrent operations are
/// handled.
pub struct Session {
    password: Vec<u8>,
    mode: Mode,
    time_diff: i64,
    challenge_timeout: f64,
    challenge_retries: u32,
    transmits: VecDeque<Vec<u8>>,
    events: VecDeque<Event>,
    op: Op,
}

/// Returns `H` followed by `tail`.
fn with_header(tail: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(HEADER.len() + tail.len());
    v.extend_from_slice(&HEADER);
    v.extend_from_slice(tail);
    v
}

impl Session {
    /// Creates a session with the given password and mode.
    ///
    /// Defaults: time difference 0, challenge timeout 0.5 s, 3 challenge tries.
    pub fn new(password: impl Into<Vec<u8>>, mode: Mode) -> Session {
        Session {
            password: password.into(),
            mode,
            time_diff: 0,
            challenge_timeout: 0.5,
            challenge_retries: 3,
            transmits: VecDeque::new(),
            events: VecDeque::new(),
            op: Op::Idle,
        }
    }

    /// Sets the mode used by later `send` calls.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// Sets the password used by later `send` calls.
    pub fn set_password(&mut self, password: impl Into<Vec<u8>>) {
        self.password = password.into();
    }

    /// Sets the signed whole-second offset added to the timestamp of
    /// time-secure packets sent by later `send` calls.
    pub fn set_time_diff(&mut self, seconds: i64) {
        self.time_diff = seconds;
    }

    /// Sets how long to wait for a challenge reply, in seconds, for later
    /// `send` calls. Default 0.5.
    pub fn set_challenge_timeout(&mut self, seconds: f64) {
        self.challenge_timeout = seconds;
    }

    /// Sets the total number of `getchallenge` sends (including the first)
    /// for later `send` calls. Default 3. A value of 0 is treated as 1.
    pub fn set_challenge_retries(&mut self, tries: u32) {
        self.challenge_retries = tries;
    }

    /// Sends an rcon command according to the current mode.
    ///
    /// In [`Mode::Nosecure`] and [`Mode::Time`] the packet is queued at once.
    /// In [`Mode::Challenge`] a `getchallenge` is queued, the command waits
    /// for the reply, and the deadline is `now + challenge timeout`.
    pub fn send(&mut self, command: &[u8], now: f64) {
        match self.mode {
            Mode::Nosecure => {
                let p = rcon_nosecure_packet(&self.password, command);
                self.transmits.push_back(p);
            }
            Mode::Time => {
                let time = now + self.time_diff as f64;
                let p = rcon_secure_time_packet(&self.password, command, time);
                self.transmits.push_back(p);
            }
            Mode::Challenge => {
                self.transmits.push_back(with_header(b"getchallenge"));
                self.op = Op::Challenge(ChallengeOp {
                    command: command.to_vec(),
                    password: self.password.clone(),
                    timeout: self.challenge_timeout,
                    tries_left: self.challenge_retries.max(1) - 1,
                    deadline: now + self.challenge_timeout,
                });
            }
        }
    }

    /// Starts a ping. Queues the ping packet; the deadline is `now + timeout`.
    ///
    /// Replaces any pending operation.
    pub fn ping(&mut self, kind: PingKind, timeout: f64, now: f64) {
        let packet = match kind {
            PingKind::Q2 => with_header(b"ping"),
            PingKind::Q3 => b"ping".to_vec(),
        };
        self.transmits.push_back(packet);
        self.op = Op::Ping {
            kind,
            start: now,
            deadline: now + timeout,
        };
    }

    /// Feeds one received datagram into the session.
    ///
    /// Valid rcon replies always produce [`Event::Response`]. Any other
    /// datagram is matched against the pending operation; datagrams that
    /// match nothing are ignored (no event, no transmit, no deadline change).
    pub fn handle_datagram(&mut self, datagram: &[u8], now: f64) {
        if let Ok(body) = parse_rcon_response(datagram) {
            self.events.push_back(Event::Response(body.to_vec()));
            return;
        }
        match std::mem::replace(&mut self.op, Op::Idle) {
            Op::Idle => {}
            Op::Challenge(c) => match parse_challenge_response(datagram) {
                Ok(challenge) => {
                    let p = rcon_secure_challenge_packet(&c.password, challenge, &c.command);
                    self.transmits.push_back(p);
                }
                Err(_) => self.op = Op::Challenge(c),
            },
            Op::Ping {
                kind,
                start,
                deadline,
            } => {
                let pong = match kind {
                    PingKind::Q2 => with_header(b"ack"),
                    PingKind::Q3 => with_header(b"disconnect"),
                };
                if datagram == pong.as_slice() {
                    self.events.push_back(Event::Pong {
                        kind,
                        rtt: now - start,
                    });
                } else {
                    self.op = Op::Ping {
                        kind,
                        start,
                        deadline,
                    };
                }
            }
        }
    }

    /// Tells the session the current time. Expires the pending operation if
    /// `now >= deadline`.
    pub fn handle_timeout(&mut self, now: f64) {
        match std::mem::replace(&mut self.op, Op::Idle) {
            Op::Idle => {}
            Op::Challenge(mut c) => {
                if now >= c.deadline {
                    if c.tries_left > 0 {
                        c.tries_left -= 1;
                        c.deadline = now + c.timeout;
                        self.transmits.push_back(with_header(b"getchallenge"));
                        self.op = Op::Challenge(c);
                    } else {
                        self.events.push_back(Event::ChallengeFailed);
                    }
                } else {
                    self.op = Op::Challenge(c);
                }
            }
            Op::Ping {
                kind,
                start,
                deadline,
            } => {
                if now >= deadline {
                    self.events.push_back(Event::PingTimeout(kind));
                } else {
                    self.op = Op::Ping {
                        kind,
                        start,
                        deadline,
                    };
                }
            }
        }
    }

    /// Returns the oldest queued outgoing datagram, if any.
    pub fn poll_transmit(&mut self) -> Option<Vec<u8>> {
        self.transmits.pop_front()
    }

    /// Returns the oldest queued event, if any.
    pub fn poll_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    /// Returns the pending deadline, or `None` when no operation is waiting.
    pub fn poll_timeout(&self) -> Option<f64> {
        match &self.op {
            Op::Idle => None,
            Op::Challenge(c) => Some(c.deadline),
            Op::Ping { deadline, .. } => Some(*deadline),
        }
    }
}
