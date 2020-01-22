#![cfg(feature = "linux_host")]

extern crate futures;
#[macro_use]
extern crate net_literals;
extern crate netsim;
#[macro_use]
extern crate unwrap;
extern crate tokio;

use futures::future;
use netsim::device::ipv4::Ipv4NatBuilder;
use netsim::{node, Ipv4Range, Network};
use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::mpsc;
use tokio::runtime::Runtime;

/// Makes 3 UDP queries from the same client to different servers and returns the ports the server
/// saw the client.
fn query_under_nat(nat_builder: Ipv4NatBuilder) -> Vec<u16> {
    let mut evloop = unwrap!(Runtime::new());
    let network = Network::new();
    let network_handle = network.handle();

    let (stun_addrs_tx, stun_addrs_rx) = mpsc::channel();
    let (client_ports_tx, client_ports_rx) = mpsc::channel();

    let mut stun_servers = vec![];
    for _ in 0..3 {
        let stun_addrs_tx = stun_addrs_tx.clone();
        let client_ports_tx = client_ports_tx.clone();

        let server = node::ipv4::machine(move |ip| {
            let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
            let sock = unwrap!(UdpSocket::bind(bind_addr));
            let pub_addr = unwrap!(sock.local_addr());
            unwrap!(stun_addrs_tx.send(pub_addr));

            let mut buf = [0; 4096];
            let (_, client_addr) = unwrap!(sock.recv_from(&mut buf));
            unwrap!(client_ports_tx.send(client_addr.port()));
            future::ok(())
        });
        stun_servers.push(server);
    }

    let client = node::ipv4::machine(move |_ip| {
        let sock = unwrap!(UdpSocket::bind(addr!("0.0.0.0:0")));
        while let Ok(addr) = stun_addrs_rx.try_recv() {
            unwrap!(sock.send_to(&[1, 2, 3], addr));
        }
        future::ok(())
    });
    let client = node::ipv4::nat(nat_builder, client);

    let router = node::ipv4::router(stun_servers);
    let router = node::ipv4::router((router, client));
    let spawn_complete = future::lazy(move || {
        let (spawn_complete, _ip_plug) =
            network_handle.spawn_ipv4_tree(Ipv4Range::global(), router);
        spawn_complete
    });
    unwrap!(evloop.block_on(spawn_complete));

    let mut ports = vec![];
    while let Ok(port) = client_ports_rx.try_recv() {
        ports.push(port);
    }
    ports
}

#[test]
fn default_nat_is_full_cone() {
    let ports = query_under_nat(Ipv4NatBuilder::default());

    assert_eq!(ports[0], ports[1]);
    assert_eq!(ports[1], ports[2]);
}

#[test]
fn symmetric_nat_assigns_different_port_for_different_endpoint() {
    let ports = query_under_nat(Ipv4NatBuilder::default().symmetric());

    assert_ne!(ports[0], ports[1]);
    assert_ne!(ports[0], ports[2]);
    assert_ne!(ports[1], ports[2]);
}
