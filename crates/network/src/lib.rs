// crates/network/src/lib.rs
//
// SDAL Network Layer
//
// Provides transport-agnostic distributed content sync protocol.
// The network crate coordinates data movement between SDAL repositories
// over HTTP (and later P2P/SSH) without holding long-term state in memory.

pub mod transport;
pub mod protocol;
pub mod client;
pub mod identity;
pub mod p2p;
pub mod wire;
