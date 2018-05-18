use priv_prelude::*;

mod latency;
mod packet_loss;
mod machine;
pub use self::latency::{latency, LatencyNode};
pub use self::packet_loss::{packet_loss, PacketLossNode};
pub use self::machine::{machine, MachineNode};

/// An `Ipv6Node` describes a recipe for constructing a network when given the IP range that the
/// network should operate on. The functions in the `node::ipv6` module return `Ipv6Node`s that you
/// can run as a network with the `spawn::network_ipv6` function.
pub trait Ipv6Node: Sized {
    /// The type returned by the thread spawned by this node.
    type Output: Send + 'static;

    /// Consume the `Ipv6Node` and build the network it describes. Returns a `SpawnComplete` that
    /// can be used to join the spawned thread and an `Ipv6Plug` that can be used to read-write
    /// packets to the head node of the network.
    fn build(
        self,
        handle: &NetworkHandle,
        ipv6_range: Ipv6Range,
    ) -> (SpawnComplete<Self::Output>, Ipv6Plug);
}

