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
    N: IpNode,
{
    PacketLossNode { node, loss_rate, mean_loss_duration }
}

impl<N> IpNode for PacketLossNode<N>
where
    N: IpNode,
{
    type Output = N::Output;

    fn build(
        self,
        handle: &NetworkHandle,
        ipv4_range: Option<Ipv4Range>,
        ipv6_range: Option<Ipv6Range>,
    ) -> (SpawnComplete<N::Output>, IpPlug) {
        let (spawn_complete, plug) = self.node.build(handle, ipv4_range, ipv6_range);
        let plug = plug.with_packet_loss(handle, self.loss_rate, self.mean_loss_duration);
        (spawn_complete, plug)
    }
}

