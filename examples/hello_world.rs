//! This is "hello world" of netsim.
//! This example demonstrates how to create a virtual network with one node and send some data
//! to it over UDP.

extern crate netsim;
extern crate tokio_core;
extern crate bytes;
extern crate futures;

use bytes::Bytes;
use futures::Future;
use futures::sync::oneshot;
use netsim::{node, spawn, Ipv4Range, Network};
use netsim::wire::{Ipv4Fields, Ipv4Packet, Ipv4PayloadFields, UdpFields};
use tokio_core::reactor::Core;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

fn main() {
    let mut evloop = Core::new().unwrap();
    let network = Network::new(&evloop.handle());

    let (server_addr_tx, server_addr_rx) = oneshot::channel();
    let server_recipe = node::ipv4::machine(|ip| {
        println!("[server] ip = {}", ip);

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
        let sock = UdpSocket::bind(bind_addr).unwrap();
        // Notify about server's IP
        let _ = server_addr_tx.send(sock.local_addr().unwrap());

        let mut buf = [0; 4096];
        let _ = sock.recv_from(&mut buf).unwrap();
        println!("[server] received: {}", String::from_utf8(buf.to_vec()).unwrap());
    });
    // Build and run server node on simulated network
    let (spawn_complete, ipv4_plug) =
        spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), server_recipe);

    let (packet_tx, _packet_rx) = ipv4_plug.split();
    // Wait till we receive server's IP address
    let server_addr = match server_addr_rx.wait().unwrap() {
        SocketAddr::V4(addr) => addr,
        _ => panic!("v6 IP was not expected"),
    };
    // Construct UDP datagram to our server node
    let datagram = Ipv4Packet::new_from_fields_recursive(
        Ipv4Fields {
            source_ip: Ipv4Addr::new(78, 1, 2, 3),
            dest_ip: *server_addr.ip(),
            ttl: 10,
        },
        Ipv4PayloadFields::Udp {
            fields: UdpFields {
                source_port: 12345,
                dest_port: server_addr.port(),
            },
            payload: Bytes::from("hello world!"),
        },
    );
    // Send datagram to our server via IPv4 plug
    let _ = packet_tx.unbounded_send(datagram).unwrap();

    // Wait till server node is finished
    evloop.run(spawn_complete).unwrap();
}
