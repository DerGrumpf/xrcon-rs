//! Integration tests against a live Xonotic server (`Session` over UDP).
//!
//! These tests are `#[ignore]`d, so `cargo test` skips them. Start a server
//! first, then run:
//!
//! ```text
//! cargo test --test integration -- --ignored
//! ```
//!
//! Server setup (from the docstring of `xrcon/tests/integration_test.py`):
//! Xonotic on localhost, port 26000, rcon password `test`, secure rcon
//! disabled:
//!
//! ```text
//! echo -e 'rcon_password "test"\nrcon_secure 0' | xonotic-dedicated -sessionid test
//! ```
//!
//! Where the expected behaviour comes from:
//! - **Python reference** (`xrcon/tests/integration_test.py`): IPv4 and IPv6
//!   on port 26000, password `test`; `status` in modes 0, 1 and 2, each reply
//!   starting with `host`; then `cvarlist sv_vote*`, collected for 0.5 s,
//!   starting with `sv_vote`.
//! - **Haskell reference** (`darkplaces-rcon/tests/DarkPlaces/RconSpec.hs`):
//!   `localhost:26000`, password `test`; `status` in the modes non-secure,
//!   time, challenge, challenge, challenge (challenge three times in a row);
//!   each reply within 1.5 s and starting with `host:`.

use std::io::ErrorKind;
use std::net::UdpSocket;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use xrcon::core::{Event, Mode, Session};

const PASSWORD: &[u8] = b"test";
/// How long the Haskell test waits for one reply (1 500 000 microseconds).
const REPLY_TIMEOUT: Duration = Duration::from_millis(1500);
/// How long the Python test collects the output of `cvarlist sv_vote*`.
const COLLECT_WINDOW: Duration = Duration::from_millis(500);
/// Maximum DarkPlaces packet size.
const MAX_PACKET_SIZE: usize = 1400;

/// The current Unix time in seconds, used as the session clock. Time-secure
/// mode needs the real timestamp, so a fake clock cannot be used here.
fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

/// A session connected to a real server through a UDP socket.
struct Live {
    socket: UdpSocket,
    session: Session,
}

impl Live {
    /// Binds a local socket at `bind` and connects it to the server at
    /// `target`. The session starts in non-secure mode.
    fn connect(bind: &str, target: &str) -> Self {
        let socket = UdpSocket::bind(bind).expect("bind local socket");
        socket.connect(target).expect("connect to server address");
        Live {
            socket,
            session: Session::new(PASSWORD, Mode::Nosecure),
        }
    }

    /// Sends every datagram the session has queued and returns its events.
    fn flush(&mut self) -> Vec<Event> {
        while let Some(datagram) = self.session.poll_transmit() {
            self.socket.send(&datagram).expect("send datagram");
        }
        let mut events = Vec::new();
        while let Some(event) = self.session.poll_event() {
            events.push(event);
        }
        events
    }

    /// Sends an rcon command using the current mode.
    fn send(&mut self, command: &[u8]) {
        self.session.send(command, now());
        let events = self.flush();
        assert!(events.is_empty(), "unexpected events on send: {events:?}");
    }

    /// Runs the session for at most `limit` and returns the bodies of the
    /// rcon replies received, in order. With `stop_at_first` it returns as
    /// soon as one reply has arrived. It panics if fetching the challenge
    /// fails.
    fn wait(&mut self, limit: Duration, stop_at_first: bool) -> Vec<Vec<u8>> {
        let deadline = Instant::now() + limit;
        let mut bodies = Vec::new();
        let mut buf = [0u8; MAX_PACKET_SIZE];
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            let wait = match self.session.poll_timeout() {
                Some(t) => left.min(Duration::from_secs_f64((t - now()).max(0.0))),
                None => left,
            };
            // A zero read timeout is an error for sockets, so use at least 1 ms.
            self.socket
                .set_read_timeout(Some(wait.max(Duration::from_millis(1))))
                .unwrap();
            match self.socket.recv(&mut buf) {
                Ok(n) => self.session.handle_datagram(&buf[..n], now()),
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    self.session.handle_timeout(now())
                }
                Err(e) => panic!("receive failed: {e}"),
            }
            for event in self.flush() {
                match event {
                    Event::Response(body) => bodies.push(body),
                    Event::ChallengeFailed => panic!("the server sent no challenge"),
                    _ => {}
                }
            }
            if stop_at_first && !bodies.is_empty() {
                break;
            }
        }
        bodies
    }

    /// Sets `mode`, runs `status`, and checks that the first reply arrives
    /// within [`REPLY_TIMEOUT`] and starts with `host:`.
    fn check_status(&mut self, mode: Mode) {
        self.session.set_mode(mode);
        self.send(b"status");
        let bodies = self.wait(REPLY_TIMEOUT, true);
        let first = bodies
            .first()
            .unwrap_or_else(|| panic!("no reply in mode {mode:?}"));
        assert!(
            first.starts_with(b"host:"),
            "mode {mode:?}: reply starts with {:?}",
            String::from_utf8_lossy(&first[..first.len().min(20)])
        );
    }

    /// Runs the whole Python reference sequence: `status` in each of the
    /// three modes, then `cvarlist sv_vote*` collected for 0.5 s.
    fn python_sequence(&mut self) {
        for mode in [Mode::Nosecure, Mode::Time, Mode::Challenge] {
            self.check_status(mode);
        }
        self.send(b"cvarlist sv_vote*");
        let collected = self.wait(COLLECT_WINDOW, false).concat();
        assert!(
            collected.starts_with(b"sv_vote"),
            "cvarlist output starts with {:?}",
            String::from_utf8_lossy(&collected[..collected.len().min(20)])
        );
    }
}

/// Python reference test over IPv4 (`127.0.0.1:26000`): `status` in modes 0,
/// 1 and 2, then `cvarlist sv_vote*`.
#[test]
#[ignore = "needs a live Xonotic server on 127.0.0.1:26000, password `test`"]
fn python_sequence_ipv4() {
    Live::connect("0.0.0.0:0", "127.0.0.1:26000").python_sequence();
}

/// Python reference test over IPv6 (`[::1]:26000`): the same sequence as
/// for IPv4.
#[test]
#[ignore = "needs a live Xonotic server on [::1]:26000, password `test`"]
fn python_sequence_ipv6() {
    Live::connect("[::]:0", "[::1]:26000").python_sequence();
}

/// Haskell reference test over IPv4: `status` in the modes non-secure, time,
/// challenge, challenge, challenge. The repeated challenge mode checks that
/// several challenge fetches in a row on one connection work.
#[test]
#[ignore = "needs a live Xonotic server on 127.0.0.1:26000, password `test`"]
fn haskell_sequence() {
    let mut live = Live::connect("0.0.0.0:0", "127.0.0.1:26000");
    for mode in [
        Mode::Nosecure,
        Mode::Time,
        Mode::Challenge,
        Mode::Challenge,
        Mode::Challenge,
    ] {
        live.check_status(mode);
    }
}
