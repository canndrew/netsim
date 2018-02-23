use priv_prelude::*;
use spawn;

// TODO: this should use a TUN interface rather than a TAP and adding an extra hop.
/// Spawn a function into a new network namespace with a single network interface with an address
/// in `subnet`. Returns a `JoinHandle` which can be used to join the spawned thread, along with
/// an `EtherBox` which can be used to read/write network activity from the spawned thread.
pub fn on_subnet_v4<F, R>(
    handle: &Handle,
    subnet: SubnetV4,
    func: F,
) -> (JoinHandle<R>, Ipv4Plug)
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static
{
    let mut iface = Ipv4IfaceBuilder::new();
    let iface_ip = subnet.random_client_addr();
    iface.address(iface_ip);
    iface.netmask(subnet.netmask());
    iface.route(RouteV4::new(subnet, None));

    let (join_handle, ipv4_plug) = spawn::with_ipv4_iface(handle, iface, move || func(iface_ip));

    (join_handle, ipv4_plug)
}

