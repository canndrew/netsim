use priv_prelude::*;

pub struct ImplNode<N> {
    node: N,
    min_latency: Duration,
    mean_additional_latency: Duration,
}

/// Add latency between nodes. Packets entering the connection from either end will be delayed
/// before arriving at the other end.
///
/// `min_latency` is the baseline for the amount of delay to add to packets.
/// `mean_additional_latency` controls the amount of random variation in the delay added to
/// packets. A non-zero `mean_additional_latency` can cause packets to be re-ordered.
pub fn latency_v4<N>(
    min_latency: Duration,
    mean_additional_latency: Duration,
    node: N,
) -> ImplNode<N>
where
    N: Ipv4Node,
{
    ImplNode { node, min_latency, mean_additional_latency }
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
    ) -> (JoinHandle<N::Output>, Ipv4Plug) {
        let (join_handle, plug) = self.node.build(handle, subnet);
        let plug = plug.with_latency(handle, self.min_latency, self.mean_additional_latency);
        (join_handle, plug)
    }
}

