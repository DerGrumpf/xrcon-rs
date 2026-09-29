//! Error type for parsing and address handling.

use std::fmt;

/// Reason a packet or address was rejected.
///
/// Every variant is a plain marker, so the type is cheap to copy and compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The packet does not start with the header required for its type.
    BadHeader,
    /// A challenge reply has fewer than 11 challenge bytes.
    ChallengeTooShort,
    /// The server-vars string is malformed.
    BadVars,
    /// A player line is malformed.
    BadPlayer,
    /// The status packet is malformed (vars line missing or invalid).
    BadStatus,
    /// A master-server entry is truncated or has a wrong separator byte.
    BadServerEntry,
    /// The master-server reply has no terminator.
    MissingTerminator,
    /// The address text is empty.
    EmptyAddr,
    /// The address has no host part.
    MissingHost,
    /// An IPv6 address was given without brackets.
    UnbracketedIpv6,
    /// A `[` has no matching `]`.
    MissingClosingBracket,
    /// The brackets contain nothing (`[]`).
    EmptyBrackets,
    /// Extra text follows the closing bracket (other than `:port`).
    TrailingAfterBracket,
    /// The port is empty, non-decimal, zero, or larger than 65535.
    InvalidPort,
    /// The address has multiple colons outside brackets and is not an IPv6 address.
    MultipleColons,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Error::BadHeader => "packet header does not match the expected type",
            Error::ChallengeTooShort => "challenge reply is shorter than 11 bytes",
            Error::BadVars => "malformed server vars",
            Error::BadPlayer => "malformed player line",
            Error::BadStatus => "malformed status packet",
            Error::BadServerEntry => "malformed master server entry",
            Error::MissingTerminator => "master server reply has no terminator",
            Error::EmptyAddr => "empty address",
            Error::MissingHost => "address has no host",
            Error::UnbracketedIpv6 => "IPv6 address must be in brackets",
            Error::MissingClosingBracket => "missing closing bracket",
            Error::EmptyBrackets => "empty brackets",
            Error::TrailingAfterBracket => "unexpected text after closing bracket",
            Error::InvalidPort => "invalid port",
            Error::MultipleColons => "multiple colons outside brackets",
        };
        f.write_str(text)
    }
}

impl std::error::Error for Error {}

/// Result alias using this crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
