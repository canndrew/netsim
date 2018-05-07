use priv_prelude::*;

/// A `Node` which adds packet loss to an underlying node.
pub struct PacketLossV4Node<N> {
    node: N,
    loss_rate: f64,
    mean_loss_duration: Duration,
}

/// Create a node which adds packet loss to the underlying `node`.
pub fn packet_loss_v4<N>(
    loss_rate: f64,
    mean_loss_duration: Duration,
    node: N,
) -> PacketLossV4Node<N>
where
    N: Ipv4Node,
{
    PacketLossV4Node { node, loss_rate, mean_loss_duration }
}

impl<N> Ipv4Node for PacketLossV4Node<N>
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
        let plug = plug.with_packet_loss(handle, self.loss_rate, self.mean_loss_duration);
        (spawn_complete, plug)
    }
}

