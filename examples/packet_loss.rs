//! This example demonstrates how to simulate packet loss on some parts of the network.

extern crate futures;
extern crate netsim;
extern crate tokio;
#[macro_use]
extern crate unwrap;
#[macro_use]
extern crate net_literals;
extern crate future_utils;
extern crate void;

use future_utils::FutureExt;
use futures::future::Loop;
use futures::sync::oneshot;
use futures::{future, stream, Future, Stream};
use netsim::node::Ipv4Node;
use netsim::{node, spawn, Ipv4Range, Network};
use tokio::net::UdpSocket;
use tokio::runtime::Runtime;
use tokio::timer::Delay;

use std::net::{SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

fn main() {
    let network = Network::new();
    let handle = network.handle();

    let mut runtime = unwrap!(Runtime::new());
    let res = runtime.block_on(future::lazy(move || {
        let (server_addr_tx, server_addr_rx) = oneshot::channel();
        let (drop_tx, drop_rx) = future_utils::drop_notify();
        let server_node = node::ipv4::machine(move |ip| {
            println!("[server] ip = {}", ip);

            let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
            let socket = unwrap!(UdpSocket::bind(&bind_addr));
            unwrap!(server_addr_tx.send(unwrap!(socket.local_addr())));

            future::loop_fn(socket, |socket| {
                socket
                    .recv_dgram(vec![0u8; 1024])
                    .map_err(|e| panic!("error receiving: {}", e))
                    .map(|(socket, buf, _len, _addr)| {
                        println!("[server] received: packet nr. {}", buf[0]);
                        Loop::Continue(socket)
                    })
            })
            .until(drop_rx)
            .map(|void_opt| match void_opt {
                Some(v) => void::unreachable(v),
                None => (),
            })
        });

        let client_node = node::ipv4::machine(|ip| {
            println!("[client] ip = {}", ip);

            server_addr_rx
                .map_err(|e| panic!("failed to get server addr: {}", e))
                .and_then(|server_addr| {
                    println!("[client] Got server addr: {}", server_addr);

                    let socket = unwrap!(UdpSocket::bind(&addr!("0.0.0.0:0")));
                    stream::iter_ok(0..10)
                        .fold(socket, move |socket, i| {
                            socket
                                .send_dgram([i], &server_addr)
                                .map_err(|e| panic!("error sending: {}", e))
                                .and_then(|(socket, _buf)| {
                                    Delay::new(Instant::now() + Duration::from_millis(500))
                                        .map(|()| socket)
                                        .map_err(|e| panic!("timer error: {}", e))
                                })
                        })
                        .map(|_socket| drop(drop_tx))
                })
        })
        .packet_loss(0.5, Duration::from_millis(500));

        let router_node = node::ipv4::router((server_node, client_node));
        let (spawn_complete, _ipv4_plug) =
            spawn::ipv4_tree(&handle, Ipv4Range::global(), router_node);

        spawn_complete.map(|((), ())| ())
    }));
    unwrap!(res)
}
