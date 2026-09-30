use std::collections::VecDeque;

use xrcon::core::{Event, Mode, Session};

/// Something the client reports to the caller.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientEvent {
    /// The reply window of a command ended; carries all reply bytes.
    Reply {
        /// The command that was sent.
        command: Vec<u8>,
        /// All reply bytes collected for it.
        body: Vec<u8>,
    },
    /// Reply to `Client::status`, parsed.
    Status(Result<crate::ServerStatus, crate::StatusError>),
    /// The server never answered the challenge request.
    ChallengeFailed,
}

struct Pending {
    command: Vec<u8>,
    body: Vec<u8>,
    /// Set once the command was actually sent (after any challenge).
    deadline: Option<f64>,
}

/// Sans-IO DarkPlaces rcon client. One command at a time.
pub struct Client {
    session: Session,
    reply_window: f64,
    pending: Option<Pending>,
    events: VecDeque<ClientEvent>,
}

impl Client {
    /// Creates a client. The reply window defaults to 0.7 s.
    pub fn new(password: impl Into<Vec<u8>>, mode: Mode) -> Client {
        Client {
            session: Session::new(password, mode),
            reply_window: 0.7,
            pending: None,
            events: VecDeque::new(),
        }
    }

    /// Sets how long replies are collected after a command was sent.
    pub fn set_reply_window(&mut self, seconds: f64) {
        self.reply_window = seconds;
    }

    /// The underlying session, for settings such as the time difference.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Sends a raw rcon command. Replaces a still pending command.
    pub fn execute(&mut self, command: &[u8], now: f64) {
        self.pending = Some(Pending {
            command: command.to_vec(),
            body: Vec::new(),
            deadline: None,
        });
        self.session.send(command, now);
        self.pump(now);
    }

    /// Feeds one received datagram.
    pub fn handle_datagram(&mut self, datagram: &[u8], now: f64) {
        self.session.handle_datagram(datagram, now);
        self.pump(now);
    }

    /// Tells the client the current time.
    pub fn handle_timeout(&mut self, now: f64) {
        self.session.handle_timeout(now);
        self.pump(now);
        let done = self
            .pending
            .as_ref()
            .is_some_and(|p| p.deadline.is_some_and(|d| now >= d));
        if done && let Some(p) = self.pending.take() {
            if p.command == crate::STATUS_COMMAND {
                self.events
                    .push_back(ClientEvent::Status(crate::ServerStatus::parse(&p.body)));
            } else {
                self.events.push_back(ClientEvent::Reply {
                    command: p.command,
                    body: p.body,
                });
            }
        }
    }

    /// Next datagram to send, if any.
    pub fn poll_transmit(&mut self) -> Option<Vec<u8>> {
        self.session.poll_transmit()
    }

    /// Next event, if any.
    pub fn poll_event(&mut self) -> Option<ClientEvent> {
        self.events.pop_front()
    }

    /// When `handle_timeout` must be called next, if anything is waiting.
    pub fn poll_timeout(&self) -> Option<f64> {
        let mine = self.pending.as_ref().and_then(|p| p.deadline);
        match (self.session.poll_timeout(), mine) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Moves session events into the pending command and starts the
    /// reply window once the command has left the challenge phase.
    fn pump(&mut self, now: f64) {
        while let Some(event) = self.session.poll_event() {
            match event {
                Event::Response(bytes) => {
                    if let Some(p) = self.pending.as_mut() {
                        p.body.extend_from_slice(&bytes);
                    }
                }
                Event::ChallengeFailed => {
                    self.pending = None;
                    self.events.push_back(ClientEvent::ChallengeFailed);
                }
                _ => {}
            }
        }
        let window = self.reply_window;
        let idle = self.session.poll_timeout().is_none();
        if let Some(p) = self.pending.as_mut()
            && p.deadline.is_none()
            && idle
        {
            p.deadline = Some(now + window);
        }
    }

    /// Sends `status 1`. The result arrives as `ClientEvent::Status`.
    pub fn status(&mut self, now: f64) {
        self.execute(crate::STATUS_COMMAND, now);
    }

    /// Sends `kick`. The server prints no reply, so the result arrives as
    /// a `ClientEvent::Reply` with an empty body.
    pub fn kick(&mut self, target: crate::KickTarget<'_>, reason: Option<&[u8]>, now: f64) {
        self.execute(&crate::kick_command(target, reason), now);
    }

    /// Sends `map`. The server restarts and kicks all players.
    pub fn map(&mut self, level: &[u8], now: f64) {
        self.execute(&crate::map_command(level), now);
    }

    /// Sends `changelevel`. Clients stay connected.
    pub fn changelevel(&mut self, level: &[u8], now: f64) {
        self.execute(&crate::changelevel_command(level), now);
    }

    /// Sends `restart`.
    pub fn restart(&mut self, now: f64) {
        self.execute(crate::RESTART_COMMAND, now);
    }

    /// Sends `maxplayers` without argument. Parse the reply with
    /// `parse_maxplayers`.
    pub fn maxplayers(&mut self, now: f64) {
        self.execute(crate::MAXPLAYERS_QUERY, now);
    }

    /// Sends `maxplayers <n>`. Applies on the next `map`.
    pub fn set_maxplayers(&mut self, n: u32, now: f64) {
        self.execute(&crate::maxplayers_set_command(n), now);
    }

    /// Sends `pause`. It toggles; there is no on/off form.
    pub fn pause(&mut self, now: f64) {
        self.execute(crate::PAUSE_COMMAND, now);
    }

    /// Sends `heartbeat`.
    pub fn heartbeat(&mut self, now: f64) {
        self.execute(crate::HEARTBEAT_COMMAND, now);
    }

    /// Sends `version`. Parse the reply with `parse_version`.
    pub fn version(&mut self, now: f64) {
        self.execute(crate::VERSION_COMMAND, now);
    }

    /// Sends `maps` with an optional prefix. Parse with `parse_maps`.
    pub fn maps(&mut self, prefix: Option<&[u8]>, now: f64) {
        self.execute(&crate::maps_command(prefix), now);
    }
}
