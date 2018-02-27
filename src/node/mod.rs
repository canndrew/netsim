//! This module is for use with the `spawn::network_v4` function. The functions herein provide a
//! simple way to define a hierarchical network with routes automatically configured. If you need
//! more flexibility in the configuration of a virtual network then you should use the `device`
//! module types directly.

use priv_prelude::*;

mod nat_v4;
mod endpoint_v4;
mod hops_v4;
mod latency_v4;
mod router_v4;

pub use self::nat_v4::nat_v4;
pub use self::endpoint_v4::endpoint_v4;
pub use self::hops_v4::hops_v4;
pub use self::latency_v4::latency_v4;
pub use self::router_v4::{router_v4, RouterClientsV4};

/// An `Ipv4Node` describes a recipe for constructing a network when given the subnet that the network
/// should operate on. The functions in the `node` module return `Ipv4Node`s that you can then run as a
/// network with the `spawn::network_v4` function.
pub trait Ipv4Node {
    /// The type returned by the thread spawned by this node.
    type Output: Send + 'static;

    /// Consume the `Ipv4Node` and build the network it describes. Returns a `JoinHandle` that can
    /// be used to join the spawned thread and an `Ipv4Plug` that can be used to read-write packets to
    /// the head node of the network.
    fn build(
        self,
        handle: &Handle,
        subnet: SubnetV4,
    ) -> (JoinHandle<Self::Output>, Ipv4Plug);
}

/// An `EtherNode` describes a recipe for constructing a network when given the subnets that the network
/// should operate on.
pub trait EtherNode {
    /// The type returned by the thread spawned by this node.
    type Output: Send + 'static;

    /// Consume the `EtherNode` and build the network it describes. Returns a `JoinHandle` that can
    /// be used to join the spawned thread and an `EtherPlug` that can be used to read-write frames to
    /// the head node of the network.
    fn build(
        self,
        handle: &Handle,
        subnet_v4: Option<SubnetV4>,
        subnet_v6: Option<SubnetV6>,
    ) -> (JoinHandle<Self::Output>, EtherPlug);
}

