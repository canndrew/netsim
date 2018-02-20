use priv_prelude::*;

pub fn on_internet<F, R>(
    handle: &Handle,
    func: F,
) -> (JoinHandle<R>, EtherBox)
where
    R: Send + 'static,
    F: FnOnce(Ipv4Addr) -> R + Send + 'static,
{
    let mut tap_builder = TapBuilderV4::new();
    let ip = Ipv4Addr::random_global();
    tap_builder.address(ip);
    trace!("ip == {}", ip);
    let route = RouteV4::new(SubnetV4::new(ip, 0), None);
    trace!("tap_builder has route {:?}", route);
    //tap_builder.route(RouteV4::new(SubnetV4::new(ipv4!("0.0.0.0"), 0), None));
    tap_builder.route(route);

    let (join_handle, tap) = with_iface(handle, tap_builder, move || func(ip));

    (join_handle, tap)
}


