use priv_prelude::*;

/// A node representing latency on an Ipv4 connection.
pub struct LatencyNode<N> {
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
pub fn latency<N>(
    min_latency: Duration,
    mean_additional_latency: Duration,
    node: N,
) -> LatencyNode<N>
where
    N: Ipv4Node,
{
    LatencyNode { node, min_latency, mean_additional_latency }
}

impl<N> Ipv4Node for LatencyNode<N>
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
        let plug = plug.with_latency(handle, self.min_latency, self.mean_additional_latency);
        (spawn_complete, plug)
    }
}

