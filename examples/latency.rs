//! This example demonstrates how to artificially introduce latency on virtual network devices.

extern crate netsim;
extern crate tokio;
extern crate futures;

use futures::Future;
use futures::sync::oneshot;
use netsim::{node, spawn, Ipv4Range, Network};
use netsim::node::Ipv4Node;
use netsim::device::ipv4::Ipv4NatBuilder;
use tokio::runtime::Runtime;

use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

fn main() {
    let mut evloop = Runtime::new().unwrap();
    let network = Network::new();
    let clock = Instant::now();

    let (server_addr_tx, server_addr_rx) = oneshot::channel();
    let server_recipe = node::ipv4::machine(move |ip| {
        println!("[server] ip = {}", ip);

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let sock = UdpSocket::bind(bind_addr).unwrap();
        let _ = server_addr_tx.send(sock.local_addr().unwrap());

        let mut buf = [0; 4096];
        let (_bytes_received, addr) = sock.recv_from(&mut buf).unwrap();
        println!(
            "[server] received: {}, from: {}, latency: {:?}",
            String::from_utf8(buf.to_vec()).unwrap(), addr, clock.elapsed(),
        );
    });

    let client_recipe = node::ipv4::machine(|ip| {
        println!("[client] ip = {}", ip);

        let server_addr = server_addr_rx.wait().unwrap();
        println!("[client] Got server addr: {}", server_addr);

        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        let _ = sock.send_to(b"hello world!", server_addr).unwrap();
    }).latency(Duration::from_secs(2), Duration::from_millis(100));
    let client_under_nat_recipe = node::ipv4::nat(Ipv4NatBuilder::default(), client_recipe);

    let router_recipe = node::ipv4::router((server_recipe, client_under_nat_recipe));
    let (spawn_complete, _ipv4_plug) =
        spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), router_recipe);

    evloop.block_on(spawn_complete).unwrap();
}
