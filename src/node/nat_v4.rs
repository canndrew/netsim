use priv_prelude::*;

pub struct ImplNode<N> {
    nat_builder: NatV4Builder,
    node: N,
}

/// Create a node for an Ipv4 NAT.
pub fn nat_v4<N>(nat_builder: NatV4Builder, node: N) -> ImplNode<N>
where
    N: Ipv4Node,
{
    ImplNode {
        nat_builder,
        node,
    }
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
    ) -> (SpawnComplete<N::Output>, Ipv4Plug) {
        let ip = subnet.random_client_addr();

        let private_subnet = {
            self.nat_builder
            .get_subnet()
            .unwrap_or_else(SubnetV4::random_local)
        };
        let nat_builder = self.nat_builder.subnet(private_subnet);
        let (spawn_complete, client_plug) = self.node.build(handle, private_subnet);
        let (public_plug_0, public_plug_1) = Ipv4Plug::new_wire();
        nat_builder.spawn(handle, public_plug_1, client_plug, ip);
        (spawn_complete, public_plug_0)
    }
}

