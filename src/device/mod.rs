//! The types in this module allow you to construct arbitrary network topologies. Have a look at
//! the `node` module if you just want to construct simple, hierarchical networks.

/// Create a namespaced thread with a set of interfaces
mod machine;
/// IPv4 devices
pub mod ipv4;
/// Ethernet devices
pub mod ether;

pub use self::machine::*;

