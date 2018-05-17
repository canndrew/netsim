use priv_prelude::*;

mod machine;
pub use self::machine::{machine, MachineNode};

/// An `IpNode` describes a recipe for constructing a network when given the IP range that the
/// network should operate on. The functions in the `node::ip` module return `IpNode`s that you
/// can run as a network with the `spawn::network_ip` function.
pub trait IpNode: Sized {
    /// The type returned by the thread spawned by this node.
    type Output: Send + 'static;

    /// Consume the `IpNode` and build the network it describes. Returns a `SpawnComplete` that
    /// can be used to join the spawned thread and an `IpPlug` that can be used to read-write
    /// packets to the head node of the network.
    fn build(
        self,
        handle: &Handle,
        ipv4_range: Option<Ipv4Range>,
        ipv6_range: Option<Ipv6Range>,
    ) -> (SpawnComplete<Self::Output>, IpPlug);
}

