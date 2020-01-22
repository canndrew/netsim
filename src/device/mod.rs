//! The types in this module allow you to construct arbitrary network topologies. Have a look at
//! the `node` module if you just want to construct simple, hierarchical networks.

/// Ethernet devices
pub mod ether;
/// IPv4 devices
pub mod ipv4;
/// IPv6 devices
pub mod ipv6;
/// Create a namespaced thread with a set of interfaces
mod machine;

pub use self::machine::*;
