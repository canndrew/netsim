//! This example demonstrates how to create a virtual network with two virtual devices, connect
//! them together with virtual router and exchange data using `std::net::UdpSocket`.

extern crate netsim;
extern crate tokio;
extern crate futures;

use futures::Future;
use futures::sync::oneshot;
use netsim::{node, spawn, Ipv4Range, Network};
use tokio::runtime::Runtime;

use std::net::{SocketAddr, SocketAddrV4, UdpSocket};

fn main() {
    let mut evloop = Runtime::new().unwrap();
    let network = Network::new();

    let (server_addr_tx, server_addr_rx) = oneshot::channel();
    let server_recipe = node::ipv4::machine(|ip| {
        println!("[server] ip = {}", ip);

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let sock = UdpSocket::bind(bind_addr).unwrap();
        let _ = server_addr_tx.send(sock.local_addr().unwrap());

        let mut buf = [0; 4096];
        let (_bytes_received, addr) = sock.recv_from(&mut buf).unwrap();
        println!("[server] received: {}, from: {}", String::from_utf8(buf.to_vec()).unwrap(), addr);
    });

    let client_recipe = node::ipv4::machine(|ip| {
        println!("[client] ip = {}", ip);

        let server_addr = server_addr_rx.wait().unwrap();
        println!("[client] Got server addr: {}", server_addr);

        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        let _ = sock.send_to(b"hello world!", server_addr).unwrap();
    });

    let router_recipe = node::ipv4::router((server_recipe, client_recipe));
    let (spawn_complete, _ipv4_plug) =
        spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), router_recipe);

    evloop.block_on(spawn_complete).unwrap();
}
