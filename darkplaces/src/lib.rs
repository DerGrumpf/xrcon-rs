//! DarkPlaces engine rcon API (sans-IO), built on `xrcon::core::Session`.

#![warn(missing_docs)]

mod chat;
mod client;
mod console;
mod control;
mod info;
mod player;

pub use client::{Client, ClientEvent};
pub use control::{
    HEARTBEAT_COMMAND, MAXPLAYERS_QUERY, PAUSE_COMMAND, RESTART_COMMAND, changelevel_command,
    map_command, maxplayers_set_command, parse_maxplayers,
};
pub use info::{
    MapEntry, STATUS_COMMAND, ServerStatus, StatusError, StatusPlayer, VERSION_COMMAND,
    maps_command, parse_maps, parse_version,
};
pub use player::{KickTarget, kick_command};
pub use xrcon::core::Mode;
