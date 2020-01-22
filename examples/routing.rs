//! This example demonstrates how to create a virtual network with two virtual devices, connect
//! them together with virtual router and exchange data using `std::net::UdpSocket`.

extern crate futures;
extern crate netsim;
extern crate tokio;
#[macro_use]
extern crate unwrap;
#[macro_use]
extern crate net_literals;

use futures::sync::oneshot;
use futures::{future, Future};
use netsim::{node, Ipv4Range, Network};
use tokio::net::UdpSocket;
use tokio::runtime::Runtime;

use std::net::{SocketAddr, SocketAddrV4};
use std::str;

fn main() {
    let network = Network::new();
    let network_handle = network.handle();

    let mut runtime = unwrap!(Runtime::new());
    let res = runtime.block_on(future::lazy(move || {
        let (server_addr_tx, server_addr_rx) = oneshot::channel();
        let server_node = node::ipv4::machine(|ip| {
            println!("[server] ip = {}", ip);

            let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
            let socket = unwrap!(UdpSocket::bind(&bind_addr));
            unwrap!(server_addr_tx.send(unwrap!(socket.local_addr())));

            socket
                .recv_dgram(vec![0u8; 1024])
                .map_err(|e| panic!("error receiving: {}", e))
                .map(|(_socket, buf, len, addr)| {
                    let s = unwrap!(str::from_utf8(&buf[..len]));
                    println!("[server] received: {}, from: {}", s, addr);
                })
        });

        let client_node = node::ipv4::machine(|ip| {
            println!("[client] ip = {}", ip);

            server_addr_rx
                .map_err(|e| panic!("failed to receive server addr: {}", e))
                .and_then(|server_addr| {
                    println!("[client] Got server addr: {}", server_addr);

                    let socket = unwrap!(UdpSocket::bind(&addr!("0.0.0.0:0")));
                    socket
                        .send_dgram(b"hello world!", &server_addr)
                        .map_err(|e| panic!("error sending: {}", e))
                        .map(|(_socket, _buf)| ())
                })
        });

        let router_node = node::ipv4::router((server_node, client_node));
        let (spawn_complete, _ipv4_plug) =
            network_handle.spawn_ipv4_tree(Ipv4Range::global(), router_node);
        spawn_complete.map(|_| ())
    }));
    unwrap!(res);
}
