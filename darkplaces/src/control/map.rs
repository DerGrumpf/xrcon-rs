/// Builds the `map` command line exactly as DarkPlaces expects it.
///
/// No checks are done. The engine wants exactly one argument, so a level
/// name with a space makes the server print its usage line. On success
/// the server restarts and kicks all players.
pub fn map_command(level: &[u8]) -> Vec<u8> {
    let mut cmd = b"map ".to_vec();
    cmd.extend_from_slice(level);
    cmd
}
