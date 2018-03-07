use priv_prelude::*;
use spawn;

/// Spawn a function into a new network namespace with a single network interface with an address
/// in `subnet`. Returns a `SpawnComplete` which can be used to join the spawned thread, along with
/// an `Ipv4Plug` which can be used to read/write network activity from the spawned thread.
pub fn on_subnet_v4<F, R>(
    handle: &Handle,
    subnet: SubnetV4,
    func: F,
) -> (SpawnComplete<R>, Ipv4Plug)
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static
{
    let iface_ip = subnet.random_client_addr();
    let iface = {
        Ipv4IfaceBuilder::new()
        .address(iface_ip)
        .netmask(subnet.netmask())
        .route(RouteV4::new(subnet, None))
    };

    let (spawn_complete, ipv4_plug) = spawn::with_ipv4_iface(handle, iface, move || func(iface_ip));

    (spawn_complete, ipv4_plug)
}

