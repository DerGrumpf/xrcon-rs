/// Builds the `changelevel` command line exactly as DarkPlaces expects it.
///
/// No checks are done. The engine wants exactly one argument. Connected
/// clients stay on the server; if no level is loaded it acts like `map`.
pub fn changelevel_command(level: &[u8]) -> Vec<u8> {
    let mut cmd = b"changelevel ".to_vec();
    cmd.extend_from_slice(level);
    cmd
}
