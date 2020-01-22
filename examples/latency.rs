//! This example demonstrates how to artificially introduce latency on virtual network devices.

extern crate futures;
extern crate netsim;
extern crate tokio;
#[macro_use]
extern crate unwrap;
#[macro_use]
extern crate net_literals;

use futures::sync::oneshot;
use futures::{future, Future};
use netsim::node::Ipv4Node;
use netsim::{node, spawn, Ipv4Range, Network};
use tokio::net::UdpSocket;
use tokio::runtime::Runtime;

use std::net::{SocketAddr, SocketAddrV4};
use std::str;
use std::time::{Duration, Instant};

fn main() {
    let network = Network::new();
    let handle = network.handle();

    let mut runtime = unwrap!(Runtime::new());
    let res = runtime.block_on(future::lazy(move || {
        let (server_addr_tx, server_addr_rx) = oneshot::channel();
        let server_node = node::ipv4::machine(move |ip| {
            println!("[server] ip = {}", ip);

            let start_time = Instant::now();
            let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
            let socket = unwrap!(UdpSocket::bind(&bind_addr));
            unwrap!(server_addr_tx.send(unwrap!(socket.local_addr())));

            socket
                .recv_dgram(vec![0u8; 1024])
                .map_err(|e| panic!("error receiving: {}", e))
                .map(move |(_socket, buf, len, addr)| {
                    let s = unwrap!(str::from_utf8(&buf[..len]));
                    let latency = start_time.elapsed();
                    println!(
                        "[server] received: {}, from: {}, latency: {:?}",
                        s, addr, latency
                    );
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
        })
        .latency(Duration::from_secs(2), Duration::from_millis(100));

        let router_node = node::ipv4::router((server_node, client_node));
        let (spawn_complete, _ipv4_plug) =
            spawn::ipv4_tree(&handle, Ipv4Range::global(), router_node);

        spawn_complete.map(|((), ())| ())
    }));
    unwrap!(res)
}
