use priv_prelude::*;
use spawn;

pub struct ImplNode<F> {
    func: F,
}

/// Create a node for an Ipv4 endpoint. This node will run the given function in a network
/// namespace with a single interface.
pub fn endpoint_v4<R, F>(func: F) -> ImplNode<F>
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static,
{
    ImplNode { func }
}

impl<R, F> Ipv4Node for ImplNode<F>
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static,
{
    type Output = R;

    fn build(
        self,
        handle: &Handle,
        subnet: SubnetV4,
    ) -> (JoinHandle<R>, Ipv4Plug) {
        let address = subnet.random_client_addr();
        let iface = {
            Ipv4IfaceBuilder::new()
            .address(address)
            .netmask(subnet.netmask())
            .route(RouteV4::new(SubnetV4::global(), None))
        };
        let (join_handle, ipv4_plug) = spawn::with_ipv4_iface(
            handle,
            iface, move || (self.func)(address),
        );
        (join_handle, ipv4_plug)
    }
}

