/// Who to kick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KickTarget<'a> {
    /// By name. DarkPlaces uses only the first word, case-insensitive.
    /// Names with spaces can only be targeted with [`KickTarget::Slot`].
    Name(&'a [u8]),
    /// By 1-based slot number.
    Slot(u32),
}

/// Builds the `kick` command line exactly as DarkPlaces expects it.
///
/// No checks are done. The server prints no reply and silently ignores
/// unknown targets and self-kicks.
pub fn kick_command(target: KickTarget<'_>, reason: Option<&[u8]>) -> Vec<u8> {
    let mut cmd = b"kick ".to_vec();
    match target {
        KickTarget::Name(name) => cmd.extend_from_slice(name),
        KickTarget::Slot(n) => cmd.extend_from_slice(format!("# {n}").as_bytes()),
    }
    if let Some(r) = reason {
        cmd.push(b' ');
        cmd.extend_from_slice(r);
    }
    cmd
}
