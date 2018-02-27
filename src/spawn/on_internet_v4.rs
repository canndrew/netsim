use priv_prelude::*;
use spawn;

/// Spawn a thread with a single network interface with a global IP address.
pub fn on_internet_v4<F, R>(
    handle: &Handle,
    func: F,
) -> (JoinHandle<R>, Ipv4Plug)
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static,
{
    let mut iface = Ipv4IfaceBuilder::new();
    let iface_ip = Ipv4Addr::random_global();
    iface.address(iface_ip);
    let route = RouteV4::new(SubnetV4::new(iface_ip, 0), None);
    iface.route(route);

    let (join_handle, ipv4_plug) = spawn::with_ipv4_iface(handle, iface, move || func(iface_ip));

    (join_handle, ipv4_plug)
}

