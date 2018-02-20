use priv_prelude::*;
use spawn;

/// Spawn a function into a new network namespace with a single network interface with an address
/// in `subnet`. Returns a `JoinHandle` which can be used to join the spawned thread, along with
/// an `EtherBox` which can be used to read/write network activity from the spawned thread.
pub fn on_subnet_v4<F, R>(
    handle: &Handle,
    subnet: SubnetV4,
    func: F,
) -> (JoinHandle<R>, EtherPlug)
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static
{
    let mut iface = IfaceBuilder::new();
    let iface_ip = subnet.random_client_addr();
    iface.address(iface_ip);
    iface.netmask(subnet.netmask());
    iface.route(RouteV4::new(subnet, None));

    let (join_handle, plug) = spawn::with_iface(handle, iface, move || func(iface_ip));

    (join_handle, plug)
}

