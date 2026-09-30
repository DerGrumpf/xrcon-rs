/// The `maxplayers` query. The server replies `"maxplayers" is "<n>"`.
pub const MAXPLAYERS_QUERY: &[u8] = b"maxplayers";

/// Builds `maxplayers <n>`. The engine clamps `n` to `1..=MAX_SCOREBOARD`
/// and applies it on the next `map`.
pub fn maxplayers_set_command(n: u32) -> Vec<u8> {
    format!("maxplayers {n}").into_bytes()
}

/// Finds the value in a `maxplayers` reply, from either the query form
/// (`"maxplayers" is "<n>"`) or the set form (`"maxplayers" set to "<n>"`).
pub fn parse_maxplayers(reply: &[u8]) -> Option<u32> {
    let text = std::str::from_utf8(reply).ok()?;
    let line = text
        .lines()
        .find(|l| l.starts_with("\"maxplayers\" is ") || l.starts_with("\"maxplayers\" set to "))?;
    let start = line.rfind("\"")?;
    let before = line.get(..start)?;
    let open = before.rfind('"')?;
    before.get(open + 1..)?.parse().ok()
}
