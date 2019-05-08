use crate::priv_prelude::*;

/// A `Node` which adds packet loss to an underlying node.
pub struct PacketLossNode<N> {
    node: N,
    loss_rate: f64,
    mean_loss_duration: Duration,
}

/// Create a node which adds packet loss to the underlying `node`.
pub fn packet_loss<N>(
    loss_rate: f64,
    mean_loss_duration: Duration,
    node: N,
) -> PacketLossNode<N>
where
    N: Ipv4Node,
{
    PacketLossNode { node, loss_rate, mean_loss_duration }
}

impl<N> Ipv4Node for PacketLossNode<N>
where
    N: Ipv4Node,
{
    type Output = N::Output;

    fn build(
        self,
        handle: &NetworkHandle,
        ipv4_range: Ipv4Range,
    ) -> (SpawnComplete<N::Output>, Ipv4Plug) {
        let (spawn_complete, plug) = self.node.build(handle, ipv4_range);
        let plug = plug.with_packet_loss(handle, self.loss_rate, self.mean_loss_duration);
        (spawn_complete, plug)
    }
}

