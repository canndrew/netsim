use priv_prelude::*;

pub struct ImplNode<N> {
    node: N,
    num_hops: u32,
}

/// Add hops between nodes. The will cause the TTL of packets travelling on this connection to
/// decrease by the given amount.
pub fn hops_v4<N>(num_hops: u32, node: N) -> ImplNode<N>
where
    N: Ipv4Node,
{
    ImplNode { node, num_hops }
}

impl<N> Ipv4Node for ImplNode<N>
where
    N: Ipv4Node,
{
    type Output = N::Output;

    fn build(
        self,
        handle: &Handle,
        subnet: SubnetV4,
    ) -> (SpawnComplete<N::Output>, Ipv4Plug) {
        let (spawn_complete, plug) = self.node.build(handle, subnet);
        let plug = plug.with_hops(handle, self.num_hops);
        (spawn_complete, plug)
    }
}

