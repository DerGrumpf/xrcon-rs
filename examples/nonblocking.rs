// Non-blocking example: calls are `async`, so other tasks can run
// while this one waits for the server.

use xrcon::core::{Mode, PingKind};
use xrcon::tokio::Client;

// Converts the number from XRCON_MODE into a Mode.
fn parse_mode(text: &str) -> Mode {
    match text {
        "0" => Mode::Nosecure,
        "1" => Mode::Time,
        "2" => Mode::Challenge,
        _ => panic!("XRCON_MODE must be 0, 1 or 2"),
    }
}

// Start the tokio runtime and run `main` on it.
#[tokio::main]
async fn main() {
    // Read the connection settings from environment variables.
    let host = std::env::var("XRCON_HOST").expect("set XRCON_HOST");
    let port: u16 = std::env::var("XRCON_PORT")
        .expect("set XRCON_PORT")
        .parse()
        .expect("XRCON_PORT must be a number");
    let password = std::env::var("XRCON_PASSWORD").expect("set XRCON_PASSWORD");
    // 0 = non-secure, 1 = time-secure, 2 = challenge-secure.
    let mode = parse_mode(&std::env::var("XRCON_MODE").expect("set XRCON_MODE"));

    // Open a UDP socket to the server (async because of the DNS lookup).
    let mut client = Client::connect(&host, port, password, mode)
        .await
        .expect("connect failed");

    // Ping the server (timeout 1 second).
    // Some(rtt) = round trip time in seconds, None = no answer in time.
    match client.ping(PingKind::Q2, 1.0).await.expect("ping failed") {
        Some(rtt) => println!("ping: {:.1} ms", rtt * 1000.0),
        None => println!("ping: timeout"),
    }

    // Run an rcon command and collect the reply for 0.7 seconds.
    // The reply comes back as raw bytes.
    let reply = client
        .execute(b"status 2", 0.7)
        .await
        .expect("execute failed");

    // Convert the bytes to text for printing.
    println!("{}", String::from_utf8_lossy(&reply));
}
