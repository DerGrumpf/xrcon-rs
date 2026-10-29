//! Blocking rcon client on a std `UdpSocket`, driving [`Session`].

use std::collections::VecDeque;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::core::{Event, Mode, PingKind, Session};

/// Errors of the blocking client.
#[derive(Debug)]
pub enum Error {
    /// Socket error.
    Io(io::Error),
    /// The server never answered the challenge request.
    ChallengeFailed,
    /// The host did not resolve.
    NoAddress,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::ChallengeFailed => f.write_str("no challenge reply from server"),
            Error::NoAddress => f.write_str("host did not resolve"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Blocking rcon client. One operation at a time.
pub struct Client {
    session: Session,
    socket: UdpSocket,
    events: VecDeque<Event>,
    buf: Vec<u8>,
}

impl Client {
    /// Resolves `host`, preferring IPv4, and connects a UDP socket.
    pub fn connect(
        host: &str,
        port: u16,
        password: impl Into<Vec<u8>>,
        mode: Mode,
    ) -> Result<Client, Error> {
        let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs()?.collect();
        let addr = addrs
            .iter()
            .find(|a| a.is_ipv4())
            .or(addrs.first())
            .copied()
            .ok_or(Error::NoAddress)?;
        let bind: SocketAddr = if addr.is_ipv4() {
            ([0u8; 4], 0).into()
        } else {
            ([0u16; 8], 0).into()
        };
        let socket = UdpSocket::bind(bind)?;
        socket.connect(addr)?;
        Ok(Client {
            session: Session::new(password, mode),
            socket,
            events: VecDeque::new(),
            buf: vec![0; 65535],
        })
    }

    /// The underlying session, for settings such as the time difference.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    fn pump(&mut self) -> Result<(), Error> {
        while let Some(p) = self.session.poll_transmit() {
            self.socket.send(&p)?;
        }
        while let Some(e) = self.session.poll_event() {
            self.events.push_back(e);
        }
        Ok(())
    }

    /// Waits up to `wait` seconds (None = forever) for one datagram.
    fn recv(&mut self, wait: Option<f64>) -> Result<(), Error> {
        let dur = match wait {
            None => None,
            Some(w) => Some(
                Duration::try_from_secs_f64(w.max(0.001))
                    .map_err(|e| Error::Io(io::Error::other(e)))?,
            ),
        };
        self.socket.set_read_timeout(dur)?;
        match self.socket.recv(&mut self.buf) {
            Ok(n) => {
                let data = self.buf.get(..n).unwrap_or(&[]);
                self.session.handle_datagram(data, now());
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                self.session.handle_timeout(now());
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
        self.pump()
    }

    fn next_event(&mut self) -> Result<Event, Error> {
        loop {
            if let Some(e) = self.events.pop_front() {
                return Ok(e);
            }
            let wait = self.session.poll_timeout().map(|d| d - now());
            self.recv(wait)?;
        }
    }

    /// Sends an rcon command and returns all reply bytes collected until
    /// `reply_window` seconds after the command was sent (after the
    /// challenge, in challenge mode).
    pub fn execute(&mut self, command: &[u8], reply_window: f64) -> Result<Vec<u8>, Error> {
        self.events.clear();
        self.session.send(command, now());
        self.pump()?;
        let mut out = Vec::new();
        let mut end: Option<f64> = None;
        loop {
            while let Some(e) = self.events.pop_front() {
                match e {
                    Event::Response(b) => out.extend_from_slice(&b),
                    Event::ChallengeFailed => return Err(Error::ChallengeFailed),
                    _ => {}
                }
            }
            let t = now();
            let deadline = self.session.poll_timeout();
            if end.is_none() && deadline.is_none() {
                end = Some(t + reply_window);
            }
            if end.is_some_and(|e| t >= e) {
                return Ok(out);
            }
            let next = [deadline, end]
                .into_iter()
                .flatten()
                .fold(f64::MAX, f64::min);
            self.recv(Some(next - t))?;
        }
    }

    /// Pings the server. Returns the round trip time in seconds, or `None`.
    pub fn ping(&mut self, kind: PingKind, timeout: f64) -> Result<Option<f64>, Error> {
        self.events.clear();
        self.session.ping(kind, timeout, now());
        self.pump()?;
        loop {
            match self.next_event()? {
                Event::Pong { rtt, .. } => return Ok(Some(rtt)),
                Event::PingTimeout(_) => return Ok(None),
                _ => {}
            }
        }
    }
}
