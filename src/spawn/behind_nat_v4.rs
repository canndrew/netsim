use priv_prelude::*;
use spawn;

/// Spawn a thread with a single interface, operating a behind a NAT.
pub fn behind_nat_v4<F, R>(
    handle: &Handle,
    nat: NatV4Builder,
    public_ip: Ipv4Addr,
    func: F,
) -> (SpawnComplete<R>, Ipv4Plug)
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let subnet = match nat.get_subnet() {
        Some(subnet) => subnet,
        None => SubnetV4::random_local(),
    };
    let nat = nat.subnet(subnet);

    let iface = {
        Ipv4IfaceBuilder::new()
        .address(subnet.random_client_addr())
        .netmask(subnet.netmask())
        .route(RouteV4::new(SubnetV4::global(), Some(subnet.gateway_ip())))
    };

    let (spawn_complete, ipv4_plug) = spawn::with_ipv4_iface(handle, iface, func);

    let (ipv4_plug_0, ipv4_plug_1) = Ipv4Plug::new_wire();
    nat.spawn(handle, ipv4_plug_1, ipv4_plug, public_ip);

    (spawn_complete, ipv4_plug_0)
}

