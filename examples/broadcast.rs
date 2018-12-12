//! This example demonstrates how to simulate LAN and broadcast packets to all machines on it.

extern crate env_logger;
extern crate futures;
#[macro_use]
extern crate net_literals;
extern crate netsim;
extern crate tokio_core;
#[macro_use]
extern crate unwrap;

use netsim::{node, Ipv4Range, Network};
use tokio_core::reactor::Core;

use std::net::{SocketAddr, SocketAddrV4, UdpSocket};

fn main() {
    unwrap!(env_logger::init());

    let mut evloop = unwrap!(Core::new());
    let network = Network::new(&evloop.handle());

    let recipe1 = node::ipv4::machine(|ip| {
        println!("[machine1] ip = {}", ip);

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 5000));
        let sock = unwrap!(UdpSocket::bind(bind_addr));

        let mut buf = [0; 4096];
        let (_bytes_received, addr) = unwrap!(sock.recv_from(&mut buf));
        println!(
            "[machine1] received: {}, from: {}",
            unwrap!(String::from_utf8(buf.to_vec())), addr
        );
    });

    let recipe2 = node::ipv4::machine(|ip| {
        println!("[machine2] ip = {}", ip);

        let sock = unwrap!(UdpSocket::bind("0.0.0.0:0"));
        unwrap!(sock.set_broadcast(true));
        let broadcast_addr = addr!("255.255.255.255:5000");
        let _ = unwrap!(sock.send_to(b"hello world!", broadcast_addr));
    });

    let router_recipe = node::ipv4::router((recipe1, recipe2));
    let (spawn_complete, _ipv4_plug) =
        network.spawn_ipv4_tree(Ipv4Range::local_subnet_192(1), router_recipe);

    evloop.run(spawn_complete).unwrap();
}
