use std::fmt;

/// Command sent by `Client::status`.
pub const STATUS_COMMAND: &[u8] = b"status 1";

/// Why a `status 1` reply could not be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StatusError {
    /// A header line is missing or malformed.
    BadHeader,
    /// A player line is malformed.
    BadPlayer,
}

impl fmt::Display for StatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            StatusError::BadHeader => "malformed status header",
            StatusError::BadPlayer => "malformed status player line",
        })
    }
}

impl std::error::Error for StatusError {}

/// One player line of `status 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusPlayer {
    /// Address as printed: `ip:port`, `hidden` or `botclient`.
    pub ip: String,
    /// Packet loss in percent.
    pub packetloss: u32,
    /// Ping in milliseconds.
    pub ping: u32,
    /// Connection time in seconds.
    pub time_secs: u32,
    /// Score.
    pub frags: i32,
    /// Client slot number (1-based).
    pub slot: u32,
    /// Raw name bytes, color codes included.
    pub name: Vec<u8>,
}

/// Parsed reply of `status 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerStatus {
    /// Host name (lossy UTF-8, color codes included).
    pub host: String,
    /// Engine version string.
    pub version: String,
    /// Protocol number.
    pub protocol_number: i32,
    /// Protocol name.
    pub protocol_name: String,
    /// Map name.
    pub map: String,
    /// Timing report as printed by the server.
    pub timing: String,
    /// Number of active players.
    pub players_active: u32,
    /// Maximum number of players.
    pub players_max: u32,
    /// Player lines.
    pub players: Vec<StatusPlayer>,
}

fn trim_start(b: &[u8]) -> &[u8] {
    let i = b.iter().position(|&c| c != b' ').unwrap_or(b.len());
    b.get(i..).unwrap_or(&[])
}

fn token(b: &[u8]) -> Option<(&[u8], &[u8])> {
    let b = trim_start(b);
    if b.is_empty() {
        return None;
    }
    let i = b.iter().position(|&c| c == b' ').unwrap_or(b.len());
    Some((b.get(..i)?, b.get(i..)?))
}

fn num<T: std::str::FromStr>(b: &[u8]) -> Option<T> {
    std::str::from_utf8(b).ok()?.parse().ok()
}

fn header<'a>(line: Option<&'a [u8]>, tag: &[u8]) -> Result<&'a [u8], StatusError> {
    line.and_then(|l| l.strip_prefix(tag))
        .map(trim_start)
        .ok_or(StatusError::BadHeader)
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn parse_time(b: &[u8]) -> Option<u32> {
    let mut it = b.split(|&c| c == b':');
    let h: u32 = num(it.next()?)?;
    let m: u32 = num(it.next()?)?;
    let s: u32 = num(it.next()?)?;
    if it.next().is_some() {
        return None;
    }
    Some(h * 3600 + m * 60 + s)
}

fn parse_player(line: &[u8]) -> Option<StatusPlayer> {
    let rest = line.get(2..)?; // color prefix ^3 or ^7
    let (ip, rest) = token(rest)?;
    let (pl, rest) = token(rest)?;
    let (ping, rest) = token(rest)?;
    let (time, rest) = token(rest)?;
    let (frags, rest) = token(rest)?;
    let (slot, rest) = token(rest)?;
    let slot = slot.strip_prefix(b"#")?;
    let name = trim_start(rest).strip_prefix(b"^7")?;
    Some(StatusPlayer {
        ip: text(ip),
        packetloss: num(pl)?,
        ping: num(ping)?,
        time_secs: parse_time(time)?,
        frags: num(frags)?,
        slot: num(slot)?,
        name: name.to_vec(),
    })
}

impl ServerStatus {
    /// Parses the reply of `status 1`.
    pub fn parse(body: &[u8]) -> Result<ServerStatus, StatusError> {
        let mut lines = body.split(|&b| b == b'\n');
        let host = header(lines.next(), b"host:")?;
        let version = header(lines.next(), b"version:")?;
        let proto = header(lines.next(), b"protocol:")?;
        let map = header(lines.next(), b"map:")?;
        let timing = header(lines.next(), b"timing:")?;
        let pl = header(lines.next(), b"players:")?;

        let (number, name) = token(proto).ok_or(StatusError::BadHeader)?;
        let protocol_number = num(number).ok_or(StatusError::BadHeader)?;
        let name = trim_start(name)
            .strip_prefix(b"(")
            .and_then(|n| n.strip_suffix(b")"))
            .ok_or(StatusError::BadHeader)?;

        let (active, rest) = token(pl).ok_or(StatusError::BadHeader)?;
        let (_, rest) = token(rest).ok_or(StatusError::BadHeader)?; // "active"
        let (max, _) = token(rest).ok_or(StatusError::BadHeader)?;
        let max = max.strip_prefix(b"(").ok_or(StatusError::BadHeader)?;

        let mut players = Vec::new();
        for line in lines {
            if line.is_empty() || line.starts_with(b"^2IP") {
                continue;
            }
            players.push(parse_player(line).ok_or(StatusError::BadPlayer)?);
        }
        Ok(ServerStatus {
            host: text(host),
            version: text(version),
            protocol_number,
            protocol_name: text(name),
            map: text(map),
            timing: text(timing),
            players_active: num(active).ok_or(StatusError::BadHeader)?,
            players_max: num(max).ok_or(StatusError::BadHeader)?,
            players,
        })
    }
}

impl fmt::Display for ServerStatus {
    /// Human-readable text form (color codes are left as they are).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Host:     {}", self.host)?;
        writeln!(f, "Version:  {}", self.version)?;
        writeln!(
            f,
            "Protocol: {} ({})",
            self.protocol_number, self.protocol_name
        )?;
        writeln!(f, "Map:      {}", self.map)?;
        writeln!(f, "Timing:   {}", self.timing)?;
        writeln!(f, "Players:  {}/{}", self.players_active, self.players_max)?;
        for p in &self.players {
            let (h, m, s) = (p.time_secs / 3600, p.time_secs / 60 % 60, p.time_secs % 60);
            writeln!(
                f,
                "  #{:<3} {} frags:{} ping:{} loss:{}% time:{}:{:02}:{:02} ip:{}",
                p.slot,
                String::from_utf8_lossy(&p.name),
                p.frags,
                p.ping,
                p.packetloss,
                h,
                m,
                s,
                p.ip
            )?;
        }
        Ok(())
    }
}
