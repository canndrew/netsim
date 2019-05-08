use crate::priv_prelude::*;

/// A node representing an Ipv4 NAT.
pub struct NatNode<N> {
    nat_builder: Ipv4NatBuilder,
    node: N,
}

/// Create a node for an Ipv4 NAT.
pub fn nat<N>(nat_builder: Ipv4NatBuilder, node: N) -> NatNode<N>
where
    N: Ipv4Node,
{
    NatNode {
        nat_builder,
        node,
    }
}

impl<N> Ipv4Node for NatNode<N>
where
    N: Ipv4Node,
{
    type Output = N::Output;

    fn build(
        self,
        handle: &NetworkHandle,
        ipv4_range: Ipv4Range,
    ) -> (SpawnComplete<N::Output>, Ipv4Plug) {
        let ip = ipv4_range.random_client_addr();

        let private_subnet = {
            self.nat_builder
            .get_subnet()
            .unwrap_or_else(Ipv4Range::random_local_subnet)
        };
        let nat_builder = self.nat_builder.subnet(private_subnet);
        let (spawn_complete, client_plug) = self.node.build(handle, private_subnet);
        let (public_plug_0, public_plug_1) = Ipv4Plug::new_pair();
        nat_builder.spawn(handle, public_plug_1, client_plug, ip);
        (spawn_complete, public_plug_0)
    }
}

