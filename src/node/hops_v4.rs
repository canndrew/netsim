use priv_prelude::*;

/// A node representing hops between Ipv4 nodes.
pub struct HopsV4Node<N> {
    node: N,
    num_hops: u32,
}

/// Add hops between nodes. The will cause the TTL of packets travelling on this connection to
/// decrease by the given amount.
pub fn hops_v4<N>(num_hops: u32, node: N) -> HopsV4Node<N>
where
    N: Ipv4Node,
{
    HopsV4Node { node, num_hops }
}

impl<N> Ipv4Node for HopsV4Node<N>
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

