use priv_prelude::*;

/// Adapts an `EtherNode` to an `Ipv4Node`
pub struct EtherAdaptorV4Node<N> {
    node: N,
}

/// Adapt a `EtherNode` into an `Ipv4Node`
pub fn ether_adaptor_v4<N>(
    node: N,
) -> EtherAdaptorV4Node<N>
where
    N: EtherNode,
{
    EtherAdaptorV4Node { node }
}

impl<N> Ipv4Node for EtherAdaptorV4Node<N>
where
    N: EtherNode,
{
    type Output = N::Output;

    fn build(
        self,
        handle: &Handle,
        subnet: SubnetV4,
    ) -> (SpawnComplete<N::Output>, Ipv4Plug) {
        let subnets = subnet.split(2);
        let (spawn_complete, ether_plug) = self.node.build(handle, Some(subnets[1]));
        let (ipv4_plug_0, ipv4_plug_1) = Ipv4Plug::new_wire();
        EtherAdaptorV4::spawn(handle, subnets[0].base_addr(), ether_plug, ipv4_plug_1);
        (spawn_complete, ipv4_plug_0)
    }
}

