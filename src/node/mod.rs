//! This module is for use with the `spawn::xxx_tree` functions. The functions herein provide a
//! simple way to define a hierarchical network with routes automatically configured. If you need
//! more flexibility in the configuration of a virtual network then you should use the `device`
//! module types directly.

/// Nodes for creating ethernet networks
pub mod ether;
/// Nodes for creating IP networks
pub mod ip;
/// Nodes for creating IPv4 networks
pub mod ipv4;
/// Nodes for creating IPv6 networks
pub mod ipv6;

pub use self::ether::EtherNode;
pub use self::ip::IpNode;
pub use self::ipv4::Ipv4Node;
pub use self::ipv6::Ipv6Node;
