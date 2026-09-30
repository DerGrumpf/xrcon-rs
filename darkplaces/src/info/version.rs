/// The `version` command line. Takes no arguments.
pub const VERSION_COMMAND: &[u8] = b"version";

/// Extracts the engine version from a `version` reply
/// (`Version: <engineversion>`). Trailing spaces in the version are kept.
pub fn parse_version(reply: &[u8]) -> Option<String> {
    let line = reply.split(|&b| b == b'\n').next()?;
    let rest = line.strip_prefix(b"Version: ")?;
    Some(String::from_utf8_lossy(rest).into_owned())
}
