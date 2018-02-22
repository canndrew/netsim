use priv_prelude::*;

pub struct ImplNode<N> {
    node: N,
    min_latency: Duration,
    mean_additional_latency: Duration,
}

pub fn latency_v4<N>(
    min_latency: Duration,
    mean_additional_latency: Duration,
    node: N,
) -> ImplNode<N>
where
    N: Node,
{
    ImplNode { node, min_latency, mean_additional_latency }
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
        let plug = plug.with_latency(handle, self.min_latency, self.mean_additional_latency);
        (join_handle, plug)
    }
}


