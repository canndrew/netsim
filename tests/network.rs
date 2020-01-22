#![cfg(feature = "linux_host")]

#[macro_use]
extern crate net_literals;
extern crate futures;
extern crate netsim;
#[macro_use]
extern crate unwrap;
extern crate tokio;

use futures::sync::oneshot;
use futures::{future, Future};
use netsim::{node, Ipv4Range, Network};
use tokio::runtime::Runtime;

#[test]
fn spawn_ipv4_tree() {
    let mut evloop = unwrap!(Runtime::new());
    let network = Network::new();
    let network_handle = network.handle();

    let (addr_tx, addr_rx) = oneshot::channel();

    let node = node::ipv4::machine(|ip| {
        unwrap!(addr_tx.send(ip));
        future::ok(())
    });
    let get_addr = future::lazy(move || {
        let (spawn_complete, _ipv4_plug) =
            network_handle.spawn_ipv4_tree(Ipv4Range::new(ipv4!("78.100.10.1"), 30), node);
        spawn_complete.and_then(|()| addr_rx.map_err(|e| panic!(e)))
    });

    let addr = unwrap!(evloop.block_on(get_addr));
    assert_eq!(addr.octets()[0..3], [78, 100, 10]);
}
