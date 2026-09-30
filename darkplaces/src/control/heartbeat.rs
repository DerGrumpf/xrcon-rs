/// The `heartbeat` command line. Takes no arguments.
///
/// The server sends a heartbeat to the master server and prints nothing.
/// If no server is running it prints
/// `No server running, can not heartbeat to master server.`
pub const HEARTBEAT_COMMAND: &[u8] = b"heartbeat";
