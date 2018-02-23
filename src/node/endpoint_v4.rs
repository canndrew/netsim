use priv_prelude::*;
use spawn;

pub struct ImplNode<F> {
    func: F,
}

pub fn endpoint_v4<R, F>(func: F) -> ImplNode<F>
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static,
{
    ImplNode { func }
}

impl<R, F> Node for ImplNode<F>
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
        let mut iface = EtherIfaceBuilder::new();
        iface.address(address);
        iface.netmask(subnet.netmask());
        iface.route(RouteV4::new(SubnetV4::global(), None));
        let (join_handle, ether_plug) = spawn::with_iface(
            handle,
            iface, move || (self.func)(address),
        );
        let (ipv4_plug_0, ipv4_plug_1) = Ipv4Plug::new_wire();
        EtherAdaptorV4::spawn(handle, subnet.random_client_addr(), ether_plug, ipv4_plug_1);
        (join_handle, ipv4_plug_0)
    }
}

