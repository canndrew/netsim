use crate::priv_prelude::*;

/// Adapts an `EtherNode` to an `Ipv4Node`
pub struct EtherAdaptorNode<N> {
    node: N,
}

/// Adapt a `EtherNode` into an `Ipv4Node`
pub fn ether_adaptor<N>(node: N) -> EtherAdaptorNode<N>
where
    N: EtherNode,
{
    EtherAdaptorNode { node }
}

impl<N> Ipv4Node for EtherAdaptorNode<N>
where
    N: EtherNode,
{
    type Output = N::Output;

    fn build(
        self,
        handle: &NetworkHandle,
        ipv4_range: Ipv4Range,
    ) -> (SpawnComplete<N::Output>, Ipv4Plug) {
        let ranges = ipv4_range.split(2);
        let (spawn_complete, ether_plug) = self.node.build(handle, Some(ranges[1]), None);
        let (ipv4_plug_0, ipv4_plug_1) = Ipv4Plug::new_pair();
        EtherAdaptorV4::spawn(handle, ranges[0].base_addr(), ether_plug, ipv4_plug_1);
        (spawn_complete, ipv4_plug_0)
    }
}
