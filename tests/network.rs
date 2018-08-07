#![cfg(feature = "linux_host")]

#[macro_use]
extern crate net_literals;
extern crate netsim;
extern crate futures;
#[macro_use]
extern crate unwrap;
extern crate tokio;

use netsim::{node, Ipv4Range, Network};
use futures::sync::oneshot;
use futures::Future;
use tokio::runtime::Runtime;

#[test]
fn spawn_ipv4_tree() {
    let mut evloop = unwrap!(Runtime::new());
    let network = Network::new();

    let (addr_tx, addr_rx) = oneshot::channel();

    let node_recipe = node::ipv4::machine(|ip| {
        unwrap!(addr_tx.send(ip));
    });
    let (spawn_complete, _ipv4_plug) = network.spawn_ipv4_tree(Ipv4Range::new(ipv4!("78.100.10.1"), 30), node_recipe);

    let addr = unwrap!(evloop.block_on(spawn_complete.and_then(|()| addr_rx.map_err(|e| panic!(e)))));
    assert_eq!(addr.octets()[0..3], [78, 100, 10]);
}
