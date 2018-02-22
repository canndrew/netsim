//! This module is for use with the `spawn::network_v4` function. The functions herein provide a
//! simple way to define a hierarchical network with routes automatically configured. If you need
//! more flexibility in the configuration of a virtual network then you should use the `device`
//! module types directly.

use priv_prelude::*;

mod nat_v4;
mod endpoint_v4;
mod hops_v4;
mod latency_v4;

pub use self::nat_v4::nat_v4;
pub use self::endpoint_v4::endpoint_v4;
pub use self::hops_v4::hops_v4;
pub use self::latency_v4::latency_v4;

/// A `Node` describes a recipe for constructing a network when given the subnet that the network
/// should operate on. The functions in the `node` module return `Node`s that you can then run as a
/// network with the `spawn::network_v4` function.
pub trait Node {
    type Output: Send + 'static;

    fn build(
        self,
        handle: &Handle,
        subnet: SubnetV4,
    ) -> (JoinHandle<Self::Output>, Ipv4Plug);
}

