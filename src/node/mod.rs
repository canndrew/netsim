//! This module is for use with the `spawn::network_v4` function. The functions herein provide a
//! simple way to define a hierarchical network with routes automatically configured. If you need
//! more flexibility in the configuration of a virtual network then you should use the `device`
//! module types directly.

use priv_prelude::*;

mod nat_v4;
mod endpoint_v4;
mod hops_v4;
mod latency_v4;
mod packet_loss_v4;
mod router_v4;

mod endpoint_eth;

pub use self::nat_v4::nat_v4;
pub use self::endpoint_v4::endpoint_v4;
pub use self::hops_v4::hops_v4;
pub use self::latency_v4::latency_v4;
pub use self::packet_loss_v4::packet_loss_v4;
pub use self::router_v4::{router_v4, RouterClientsV4};

pub use self::endpoint_eth::endpoint_eth;

/// An `Ipv4Node` describes a recipe for constructing a network when given the subnet that the network
/// should operate on. The functions in the `node` module return `Ipv4Node`s that you can then run as a
/// network with the `spawn::network_v4` function.
pub trait Ipv4Node: Sized {
    /// The type returned by the thread spawned by this node.
    type Output: Send + 'static;

    /// Consume the `Ipv4Node` and build the network it describes. Returns a `SpawnComplete` that can
    /// be used to join the spawned thread and an `Ipv4Plug` that can be used to read-write packets to
    /// the head node of the network.
    fn build(
        self,
        handle: &Handle,
        subnet: SubnetV4,
    ) -> (SpawnComplete<Self::Output>, Ipv4Plug);

    /// Chain some extra hops onto the node, causing TTL values of packets to decrease by
    /// `num_hops` on their way to/from the node.
    fn hops(
        self,
        num_hops: u32,
    ) -> hops_v4::ImplNode<Self> {
        hops_v4(num_hops, self)
    }

    /// Add latency to the node. Packets on their way to/from the node will be delayed by
    /// `min_latency + r` where `r` is a random amount with mean `mean_additional_latency`.
    fn latency(
        self,
        min_latency: Duration,
        mean_additional_latency: Duration,
    ) -> latency_v4::ImplNode<Self> {
        latency_v4(min_latency, mean_additional_latency, self)
    }

    /// Add packet loss to a node. Loss happens in burst, rather than on an individual packet
    /// basis. `mean_loss_duration` controls the burstiness of the loss. 
    fn packet_loss(
        self,
        loss_rate: f64,
        mean_loss_duration: Duration,
    ) -> packet_loss_v4::ImplNode<Self> {
        packet_loss_v4(loss_rate, mean_loss_duration, self)
    }
}

/// An `EtherNode` describes a recipe for constructing a network when given the subnets that the network
/// should operate on.
pub trait EtherNode: Sized {
    /// The type returned by the thread spawned by this node.
    type Output: Send + 'static;

    /// Consume the `EtherNode` and build the network it describes. Returns a `SpawnComplete` that can
    /// be used to join the spawned thread and an `EtherPlug` that can be used to read-write frames to
    /// the head node of the network.
    fn build(
        self,
        handle: &Handle,
        subnet_v4: Option<SubnetV4>,
    ) -> (SpawnComplete<Self::Output>, EtherPlug);
}

