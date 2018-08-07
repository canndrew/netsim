extern crate netsim;
extern crate tokio;
extern crate futures;
extern crate env_logger;

use futures::Future;
use futures::sync::oneshot;
use netsim::{node, spawn, Ipv4Range, Network};
use netsim::device::ipv4::Ipv4NatBuilder;
use tokio::runtime::Runtime;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

fn main() {
    ::env_logger::init().unwrap();

    let mut evloop = Runtime::new().unwrap();
    let network = Network::new();

    let (server_addr_tx, server_addr_rx) = oneshot::channel();
    let (client1_addr_tx, client1_addr_rx) = oneshot::channel();
    let server_recipe = node::ipv4::machine(|ip| {
        println!("[server] ip = {}", ip);

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let sock = UdpSocket::bind(bind_addr).unwrap();
        let _ = server_addr_tx.send(sock.local_addr().unwrap());

        let mut buf = [0; 4096];
        let (_bytes_received, addr) = sock.recv_from(&mut buf).unwrap();
        println!("[server] received: {}, from: {}", String::from_utf8(buf.to_vec()).unwrap(), addr);

        let _ = client1_addr_tx.send(addr);
    });

    let client1_recipe = node::ipv4::machine(|ip| {
        println!("[client1] ip = {}", ip);

        let server_addr = server_addr_rx.wait().unwrap();
        println!("[client1] Got server addr: {}", server_addr);

        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        let _ = sock.send_to(b"hello world!", server_addr).unwrap();

        let mut buf = [0; 4096];
        let (_bytes_received, addr) = sock.recv_from(&mut buf).unwrap();
        println!("[client1] received: {}, from: {}", String::from_utf8(buf.to_vec()).unwrap(), addr);
    });
    let nat_builder = Ipv4NatBuilder::new()
        .subnet(Ipv4Range::new(Ipv4Addr::new(192, 168, 1, 0), 24));
    let client1_under_nat_recipe = node::ipv4::nat(nat_builder, client1_recipe);

    let client2_recipe = node::ipv4::machine(|ip| {
        println!("[client2] ip = {}", ip);

        let client1_addr = client1_addr_rx.wait().unwrap();
        println!("[client2] Got client1 addr: {}", client1_addr);

        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        let _ = sock.send_to(b"this is client2!", client1_addr).unwrap();
    });

    let router_recipe = node::ipv4::router((server_recipe, client1_under_nat_recipe, client2_recipe));
    let (spawn_complete, _ipv4_plug) =
        spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), router_recipe);

    evloop.block_on(spawn_complete).unwrap();
}
