//! Tokio client for the DarkPlaces rcon protocol, driving `xrcon::core::Session`.

#![warn(missing_docs)]

use std::collections::VecDeque;
use std::io;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::net::{UdpSocket, lookup_host};
use tokio::time::timeout as tokio_timeout;
use xrcon::core::{Event, Mode, PingKind, Session};
use xrcon::parse::Player;

pub use xrcon::core::{Event as SessionEvent, Mode as RconMode};

/// Errors of the Tokio client.
#[derive(Debug)]
pub enum Error {
    /// Socket error.
    Io(io::Error),
    /// The server never answered the challenge request.
    ChallengeFailed,
    /// The status reply could not be parsed.
    StatusInvalid,
    /// The host did not resolve.
    NoAddress,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::ChallengeFailed => f.write_str("no challenge reply from server"),
            Error::StatusInvalid => f.write_str("invalid status reply"),
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

/// Result of a status query.
pub type Status = (std::collections::HashMap<Vec<u8>, Vec<u8>>, Vec<Player>);

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Async rcon client. One operation at a time (`&mut self`).
pub struct Client {
    session: Session,
    socket: UdpSocket,
    events: VecDeque<Event>,
    buf: Vec<u8>,
}

impl Client {
    /// Connects a UDP socket to `host:port`.
    pub async fn connect(
        host: &str,
        port: u16,
        password: impl Into<Vec<u8>>,
        mode: Mode,
    ) -> Result<Client, Error> {
        let addr = lookup_host((host, port))
            .await?
            .find(|a| a.is_ipv4())
            .or(lookup_host((host, port)).await?.next())
            .ok_or(Error::NoAddress)?;
        let bind = if addr.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        let socket = UdpSocket::bind(bind).await?;
        socket.connect(addr).await?;
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

    async fn pump(&mut self) -> Result<(), Error> {
        while let Some(p) = self.session.poll_transmit() {
            self.socket.send(&p).await?;
        }
        while let Some(e) = self.session.poll_event() {
            self.events.push_back(e);
        }
        Ok(())
    }

    /// Waits up to `wait` seconds (None = forever) for one datagram.
    async fn recv(&mut self, wait: Option<f64>) -> Result<(), Error> {
        let fut = self.socket.recv(&mut self.buf);
        let got = match wait {
            None => Some(fut.await?),
            Some(w) => match tokio_timeout(Duration::from_secs_f64(w.max(0.001)), fut).await {
                Ok(r) => Some(r?),
                Err(_) => None,
            },
        };
        match got {
            Some(n) => {
                let data = self.buf.get(..n).unwrap_or(&[]).to_vec();
                self.session.handle_datagram(&data, now());
            }
            None => self.session.handle_timeout(now()),
        }
        self.pump().await
    }

    async fn next_event(&mut self) -> Result<Event, Error> {
        loop {
            if let Some(e) = self.events.pop_front() {
                return Ok(e);
            }
            let wait = self.session.poll_timeout().map(|d| d - now());
            self.recv(wait).await?;
        }
    }

    /// Sends an rcon command and returns all reply bytes collected until
    /// `reply_window` seconds after the command was sent (after the
    /// challenge, in challenge mode).
    pub async fn execute(&mut self, command: &[u8], reply_window: f64) -> Result<Vec<u8>, Error> {
        self.events.clear();
        self.session.send(command, now());
        self.pump().await?;
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
            self.recv(Some(next - t)).await?;
        }
    }

    /// Pings the server. Returns the round trip time in seconds, or `None`
    /// on timeout.
    pub async fn ping(&mut self, kind: PingKind, timeout: f64) -> Result<Option<f64>, Error> {
        self.events.clear();
        self.session.ping(kind, timeout, now());
        self.pump().await?;
        loop {
            match self.next_event().await? {
                Event::Pong { rtt, .. } => return Ok(Some(rtt)),
                Event::PingTimeout(_) => return Ok(None),
                _ => {}
            }
        }
    }

    /// Queries the server status. Returns `None` on timeout.
    pub async fn getstatus(&mut self, timeout: f64) -> Result<Option<Status>, Error> {
        self.events.clear();
        self.session.getstatus(timeout, now());
        self.pump().await?;
        loop {
            match self.next_event().await? {
                Event::Status { vars, players } => return Ok(Some((vars, players))),
                Event::StatusTimeout => return Ok(None),
                Event::StatusInvalid => return Err(Error::StatusInvalid),
                _ => {}
            }
        }
    }
}
