use priv_prelude::*;

pub struct ImplNode<N> {
    node: N,
    num_hops: u32,
}

pub fn hops_v4<N>(num_hops: u32, node: N) -> ImplNode<N>
where
    N: Node,
{
    ImplNode { node, num_hops }
}

impl<N> Node for ImplNode<N>
where
    N: Node,
{
    type Output = N::Output;

    fn build(
        self,
        handle: &Handle,
        subnet: SubnetV4,
    ) -> (JoinHandle<N::Output>, Ipv4Plug) {
        let (join_handle, plug) = self.node.build(handle, subnet);
        let plug = plug.with_hops(handle, self.num_hops);
        (join_handle, plug)
    }
}

