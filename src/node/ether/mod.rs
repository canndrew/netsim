use priv_prelude::*;

mod endpoint;
mod hub;

pub use self::endpoint::{endpoint, EndpointNode};
pub use self::hub::{hub, HubNode};

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

