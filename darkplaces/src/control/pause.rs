/// The `pause` command line. Takes no arguments.
///
/// It toggles the paused state; there is no explicit on/off form.
/// The server broadcasts the change to players. No reply is documented
/// for the rcon caller.
pub const PAUSE_COMMAND: &[u8] = b"pause";
