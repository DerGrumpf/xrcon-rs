//! `xrcon` command line client. Same options as the Python `xrcon` tool.

use std::io::IsTerminal;
use std::io::{self, Write};
use std::path::PathBuf;

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use configparser::ini::Ini;

use xrcon::color;
use xrcon::core::Mode;
use xrcon::net::Client;
use xrcon::parse::parse_server_addr;

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
    let mut client = Client::connect(&host, port, password.into_bytes(), mode)
        .map_err(|e| format!("can't connect to '{host}': {e}"))?;
    if mode == Mode::Challenge {
        client.session_mut().set_challenge_timeout(timeout);
        client.session_mut().set_challenge_retries(1);
    }
    let out = client
        .execute(cli.command.join(" ").as_bytes(), timeout)
        .map_err(|e| e.to_string())?;
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
