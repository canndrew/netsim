use priv_prelude::*;

mod nat_v4;
mod endpoint_v4;
mod hops_v4;
mod latency_v4;

pub use self::nat_v4::nat_v4;
pub use self::endpoint_v4::endpoint_v4;
pub use self::hops_v4::hops_v4;
pub use self::latency_v4::latency_v4;

pub trait Node {
    type Output: Send + 'static;

    fn build(
        self,
        handle: &Handle,
        subnet: SubnetV4,
    ) -> (JoinHandle<Self::Output>, Ipv4Plug);
}

