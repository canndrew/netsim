//! This module is for use with the `spawn::network_v4` function. The functions herein provide a
//! simple way to define a hierarchical network with routes automatically configured. If you need
//! more flexibility in the configuration of a virtual network then you should use the `device`
//! module types directly.

/// Nodes for creating IPv4 networks
pub mod ipv4;
/// Nodes for creating ethernet networks
pub mod ether;

pub use self::ipv4::Ipv4Node;
pub use self::ether::EtherNode;

