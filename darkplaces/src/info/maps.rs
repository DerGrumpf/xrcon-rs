/// One map line of a `maps` reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapEntry {
    /// Map name without `maps/` and `.bsp`.
    pub name: String,
    /// Format tag, e.g. `Q3BSP46`, `BSP29`, `VBSP20`, `unknown<n>`.
    pub format: String,
    /// Title from worldspawn, or an engine error text. May be empty.
    pub title: Vec<u8>,
}

/// Builds `maps` or `maps <prefix>`. The engine accepts at most one
/// argument; no checks are done here.
pub fn maps_command(prefix: Option<&[u8]>) -> Vec<u8> {
    let mut cmd = b"maps".to_vec();
    if let Some(p) = prefix {
        cmd.push(b' ');
        cmd.extend_from_slice(p);
    }
    cmd
}

fn parse_line(line: &[u8]) -> Option<MapEntry> {
    let start = line.iter().position(|&b| b != b' ')?;
    let line = line.get(start..)?;
    let open = line.windows(2).position(|w| w == b" (")?;
    let name = line.get(..open)?;
    let rest = line.get(open + 2..)?;
    let close = rest.windows(2).position(|w| w == b") ")?;
    let format = rest.get(..close)?.trim_ascii_end();
    let title = rest.get(close + 2..)?;
    Some(MapEntry {
        name: String::from_utf8_lossy(name).into_owned(),
        format: String::from_utf8_lossy(format).into_owned(),
        title: title.to_vec(),
    })
}

/// Parses a `maps` reply. The `<n> maps found :` header, the trailing
/// empty line and the usage line are skipped; lines that do not match
/// the map format are ignored.
pub fn parse_maps(reply: &[u8]) -> Vec<MapEntry> {
    reply
        .split(|&b| b == b'\n')
        .filter(|l| !l.starts_with(b"usage:"))
        .filter_map(parse_line)
        .collect()
}
