//! `xrcon` command line client. Same options as the Python `xrcon` tool.

use std::io::IsTerminal;
use std::io::{self, Write};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use configparser::ini::Ini;

use xrcon::core::{Event, Mode, Session};
use xrcon::parse::parse_server_addr;

mod color;

const DEFAULT_PORT: u16 = 26000;
const DEFAULT_TIMEOUT: f64 = 0.7;
const DEFAULT_TYPE: u8 = 1;

/// Executes rcon command
#[derive(Parser)]
#[command(name = "xrcon", about = "Executes rcon command")]
struct Cli {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    timeout: Option<f64>,
    #[arg(short = 'n', long)]
    name: Option<String>,
    #[arg(short, long)]
    server: Option<String>,
    #[arg(short, long)]
    password: Option<String>,
    #[arg(
        short = 't',
        long = "type",
        value_name = "{0,1,2}",
        help = "Connection type: 0=non-secure, 1=time-secure, 2=challenge-secure [default: 1]",
        value_parser = clap::value_parser!(u8).range(0..=2)
    )]
    kind: Option<u8>,
    #[arg(required = true, num_args = 1..)]
    command: Vec<String>,
}

/// Prints usage and the message to stderr, then exits with code 2.
fn fail(message: String) -> ! {
    Cli::command().error(ErrorKind::Io, message).exit()
}

/// Current Unix time in seconds.
fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Loads `--config`, or `~/.xrcon.ini` if it exists.
fn load_config(path: Option<&PathBuf>) -> Result<Ini, String> {
    let mut ini = Ini::new();
    match path {
        Some(p) => {
            ini.load(p)
                .map_err(|e| format!("can't read config '{}': {e}", p.display()))?;
        }
        None => {
            if let Some(p) = home_dir()
                .map(|h| h.join(".xrcon.ini"))
                .filter(|p| p.is_file())
            {
                ini.load(&p)
                    .map_err(|e| format!("can't read config '{}': {e}", p.display()))?;
            }
        }
    }
    Ok(ini)
}

/// Resolves a host, preferring an IPv4 result, else the first one.
fn resolve(host: &str, port: u16) -> io::Result<SocketAddr> {
    let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs()?.collect();
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or(addrs.first())
        .copied()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no address found"))
}

fn run(cli: &Cli) -> Result<(), String> {
    let ini = load_config(cli.config.as_ref())?;
    let section = cli
        .name
        .as_deref()
        .map_or_else(|| "default".to_string(), str::to_lowercase);
    if cli.name.is_some() && !ini.get_map_ref().contains_key(&section) {
        return Err(format!("no section '{section}' in config"));
    }
    let get = |key: &str| ini.get(&section, key).or_else(|| ini.get("default", key));

    let server = cli
        .server
        .clone()
        .or_else(|| get("server"))
        .ok_or("no server given: use -s or the config file")?;

    let password = match cli.password.clone().or_else(|| get("password")) {
        Some(p) => p,
        None => {
            eprint!("Password: ");
            io::stderr().flush().map_err(|e| e.to_string())?;
            rpassword::read_password().map_err(|e| format!("can't read password: {e}"))?
        }
    };

    let kind = match cli.kind {
        Some(k) => k,
        None => match get("type") {
            Some(t) => t
                .trim()
                .parse::<u8>()
                .map_err(|_| "invalid rcon type in config".to_string())?,
            None => DEFAULT_TYPE,
        },
    };
    let mode = match kind {
        0 => Mode::Nosecure,
        1 => Mode::Time,
        2 => Mode::Challenge,
        _ => return Err("invalid rcon type".to_string()),
    };

    let timeout = match cli.timeout {
        Some(t) => t,
        None => match get("timeout") {
            Some(t) => t
                .trim()
                .parse::<f64>()
                .map_err(|_| "invalid timeout in config".to_string())?,
            None => DEFAULT_TIMEOUT,
        },
    };
    if !timeout.is_finite() || timeout < 0.0 {
        return Err("timeout must be a finite number >= 0".to_string());
    }

    let (host, port) = parse_server_addr(&server, DEFAULT_PORT)
        .map_err(|e| format!("bad server address '{server}': {e}"))?;
    let addr = resolve(&host, port).map_err(|e| format!("can't resolve '{host}': {e}"))?;
    let bind: SocketAddr = if addr.is_ipv4() {
        ([0u8; 4], 0).into()
    } else {
        ([0u16; 8], 0).into()
    };
    let socket = UdpSocket::bind(bind).map_err(|e| e.to_string())?;
    socket.connect(addr).map_err(|e| e.to_string())?;

    let mut session = Session::new(password.into_bytes(), mode);
    if mode == Mode::Challenge {
        session.set_challenge_timeout(timeout);
        session.set_challenge_retries(1);
    }
    session.send(cli.command.join(" ").as_bytes(), now());

    let mut out: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; 65535];
    let mut read_until: Option<f64> = None;
    loop {
        while let Some(packet) = session.poll_transmit() {
            socket.send(&packet).map_err(|e| e.to_string())?;
        }
        while let Some(event) = session.poll_event() {
            match event {
                Event::Response(body) => out.extend_from_slice(&body),
                Event::ChallengeFailed => {
                    return Err("no challenge reply from server".to_string());
                }
                _ => {}
            }
        }
        let t = now();
        let deadline = match session.poll_timeout() {
            Some(d) => d,
            None => {
                let r = *read_until.get_or_insert(t + timeout);
                if t >= r {
                    break;
                }
                r
            }
        };
        let wait = deadline - t;
        if wait <= 0.0 {
            session.handle_timeout(t);
            continue;
        }
        let dur = Duration::try_from_secs_f64(wait.max(0.001)).map_err(|e| e.to_string())?;
        socket
            .set_read_timeout(Some(dur))
            .map_err(|e| e.to_string())?;
        match socket.recv(&mut buf) {
            Ok(n) => session.handle_datagram(buf.get(..n).unwrap_or(&[]), now()),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                session.handle_timeout(now());
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.to_string()),
        }
    }

    let mut stdout = io::stdout();
    let data = if stdout.is_terminal() {
        color::colorize(&out)
    } else {
        out
    };
    stdout.write_all(&data).map_err(|e| e.to_string())?;
    stdout.flush().map_err(|e| e.to_string())
}

fn main() {
    let cli = Cli::parse();
    if let Err(message) = run(&cli) {
        fail(message);
    }
}
