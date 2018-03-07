use priv_prelude::*;
use spawn;

/// Spawn a thread with a single network interface with a global IP address.
pub fn on_internet_v4<F, R>(
    handle: &Handle,
    func: F,
) -> (SpawnComplete<R>, Ipv4Plug)
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static,
{
    let iface_ip = Ipv4Addr::random_global();
    let route = RouteV4::new(SubnetV4::new(iface_ip, 0), None);
    let iface = {
        Ipv4IfaceBuilder::new()
        .address(iface_ip)
        .route(route)
    };

    let (spawn_complete, ipv4_plug) = spawn::with_ipv4_iface(handle, iface, move || func(iface_ip));

    (spawn_complete, ipv4_plug)
}

