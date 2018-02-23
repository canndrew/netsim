use priv_prelude::*;
use spawn;

// TODO: this should use a TUN interface rather than a TAP and adding an extra hop.
pub fn on_internet_v4<F, R>(
    handle: &Handle,
    func: F,
) -> (JoinHandle<R>, Ipv4Plug)
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static,
{
    let mut iface = EtherIfaceBuilder::new();
    let iface_ip = Ipv4Addr::random_global();
    iface.address(iface_ip);
    let route = RouteV4::new(SubnetV4::new(iface_ip, 0), None);
    iface.route(route);

    let (join_handle, ether_plug) = spawn::with_iface(handle, iface, move || func(iface_ip));
    let (ipv4_plug_0, ipv4_plug_1) = Ipv4Plug::new_wire();

    EtherAdaptorV4::spawn(
        handle,
        Ipv4Addr::random_global(),
        ether_plug,
        ipv4_plug_0,
    );

    (join_handle, ipv4_plug_1)
}

