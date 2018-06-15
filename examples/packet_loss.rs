//! This example demonstrates how to simulate packet loss on some parts of the network.

extern crate netsim;
extern crate tokio_core;
extern crate futures;

use futures::Future;
use futures::sync::oneshot;
use netsim::{node, spawn, Ipv4Range, Network};
use netsim::node::Ipv4Node;
use tokio_core::reactor::Core;

use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::time::Duration;
use std::thread::sleep;

fn main() {
    let mut evloop = Core::new().unwrap();
    let network = Network::new(&evloop.handle());

    let (server_addr_tx, server_addr_rx) = oneshot::channel();
    let server_recipe = node::ipv4::machine(move |ip| {
        println!("[server] ip = {}", ip);

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let sock = UdpSocket::bind(bind_addr).unwrap();
        let _ = server_addr_tx.send(sock.local_addr().unwrap());

        let mut buf = [0; 4096];
        loop {
            let _  = sock.recv_from(&mut buf).unwrap();
            println!("[server] received: packet nr. {}", buf[0]);
        }
    });

    let client_recipe = node::ipv4::machine(|ip| {
        println!("[client] ip = {}", ip);

        let server_addr = server_addr_rx.wait().unwrap();
        println!("[client] Got server addr: {}", server_addr);

        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        for i in 1..11 {
            let _ = sock.send_to(&[i], server_addr).unwrap();
            sleep(Duration::from_millis(500));
        }
    }).packet_loss(0.5, Duration::from_millis(500));

    let router_recipe = node::ipv4::router((server_recipe, client_recipe));
    let (spawn_complete, _ipv4_plug) =
        spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), router_recipe);

    evloop.run(spawn_complete).unwrap();
}
