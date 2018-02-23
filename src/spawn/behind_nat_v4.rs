use priv_prelude::*;
use spawn;

pub fn behind_nat_v4<F, R>(
    handle: &Handle,
    nat: NatV4Builder,
    public_ip: Ipv4Addr,
    func: F,
) -> (JoinHandle<R>, Ipv4Plug)
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let subnet = match nat.get_subnet() {
        Some(subnet) => subnet,
        None => SubnetV4::random_local(),
    };
    let nat = nat.subnet(subnet);

    let mut iface = EtherIfaceBuilder::new();
    iface.address(subnet.random_client_addr());
    iface.netmask(subnet.netmask());
    iface.route(RouteV4::new(SubnetV4::global(), Some(subnet.gateway_ip())));

    let (join_handle, ether_plug) = spawn::with_iface(handle, iface, func);

    let (ipv4_plug_a0, ipv4_plug_a1) = Ipv4Plug::new_wire();
    EtherAdaptorV4::spawn(
        handle,
        subnet.gateway_ip(),
        ether_plug,
        ipv4_plug_a0,
    );

    let (ipv4_plug_b0, ipv4_plug_b1) = Ipv4Plug::new_wire();
    nat.spawn(handle, ipv4_plug_b1, ipv4_plug_a1, public_ip);

    (join_handle, ipv4_plug_b0)
}

