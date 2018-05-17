use priv_prelude::*;

mod nat;
mod machine;
mod hops;
mod latency;
mod packet_loss;
mod router;
mod ether_adaptor;

pub use self::nat::{nat, NatNode};
pub use self::machine::{machine, MachineNode};
pub use self::hops::{hops, HopsNode};
pub use self::latency::{latency, LatencyNode};
pub use self::packet_loss::{packet_loss, PacketLossNode};
pub use self::router::{router, RouterClientsV4, RouterNode};
pub use self::ether_adaptor::{ether_adaptor, EtherAdaptorNode};

/// An `Ipv4Node` describes a recipe for constructing a network when given the IP range that the
/// network should operate on. The functions in the `node::ipv4` module return `Ipv4Node`s that you
/// can run as a network with the `spawn::network_v4` function.
pub trait Ipv4Node: Sized {
    /// The type returned by the thread spawned by this node.
    type Output: Send + 'static;

    /// Consume the `Ipv4Node` and build the network it describes. Returns a `SpawnComplete` that
    /// can be used to join the spawned thread and an `Ipv4Plug` that can be used to read-write
    /// packets to the head node of the network.
    fn build(
        self,
        handle: &Handle,
        ipv4_range: Ipv4Range,
    ) -> (SpawnComplete<Self::Output>, Ipv4Plug);

    /// Chain some extra hops onto the node, causing TTL values of packets to decrease by
    /// `num_hops` on their way to/from the node.
    fn hops(
        self,
        num_hops: u32,
    ) -> HopsNode<Self> {
        hops(num_hops, self)
    }

    /// Add latency to the node. Packets on their way to/from the node will be delayed by
    /// `min_latency + r` where `r` is a random amount with mean `mean_additional_latency`.
    fn latency(
        self,
        min_latency: Duration,
        mean_additional_latency: Duration,
    ) -> LatencyNode<Self> {
        latency(min_latency, mean_additional_latency, self)
    }

    /// Add packet loss to a node. Loss happens in burst, rather than on an individual packet
    /// basis. `mean_loss_duration` controls the burstiness of the loss. 
    fn packet_loss(
        self,
        loss_rate: f64,
        mean_loss_duration: Duration,
    ) -> PacketLossNode<Self> {
        packet_loss(loss_rate, mean_loss_duration, self)
    }
}

