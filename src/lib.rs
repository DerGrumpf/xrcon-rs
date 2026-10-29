//! DarkPlaces rcon protocol.
//!
//! The crate is sans-IO: [`packet`] builds outgoing packets, [`parse`]
//! parses incoming ones, and [`core::Session`] drives a client without
//! touching sockets or clocks.

#![warn(missing_docs)]

pub mod color;
pub mod core;
pub mod error;
pub mod net;
pub mod packet;
pub mod parse;

#[cfg(feature = "tokio")]
pub mod tokio;

pub use error::{Error, Result};
