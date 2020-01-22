//! This a basic example how to setup NAT (Network Address Translation).
//!
//! 1. it creates publicly accessible server node.
//! 2. client node is created and put under NAT
//! 3. client connects to the server
//!
//! When you run the example, you can see that client node sees its LAN IP address and when it
//! connects to the server, server sees its public IP - one that NAT device owns.

extern crate future_utils;
extern crate futures;
extern crate netsim;
extern crate tokio;
#[macro_use]
extern crate unwrap;

use futures::sync::oneshot;
use futures::Stream;
use futures::{future, Future};
use netsim::device::ipv4::Ipv4NatBuilder;
use netsim::{node, spawn, Ipv4Range, Network};
use std::net::{SocketAddr, SocketAddrV4};
use std::thread;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;

fn main() {
    let network = Network::new();
    let handle = network.handle();

    let mut runtime = unwrap!(Runtime::new());
    let res = runtime.block_on(future::lazy(move || {
        let (server_addr_tx, server_addr_rx) = oneshot::channel();
        let server = node::ipv4::machine(move |ip| {
            // This code is run on a separate thread.
            println!(
                "[server] ip = {}, thread = {:?}",
                ip,
                thread::current().id()
            );

            let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
            let listener = unwrap!(TcpListener::bind(&bind_addr));
            unwrap!(server_addr_tx.send(unwrap!(listener.local_addr())));

            listener
                .incoming()
                .into_future()
                .map_err(|(e, _incoming)| panic!("error accpting connection: {}", e))
                .map(|(stream_opt, _incoming)| {
                    let stream = unwrap!(stream_opt);
                    let addr = unwrap!(stream.peer_addr());
                    println!("[server] Client connected: {}", addr);
                })
        });
        let client = node::ipv4::machine(move |ip| {
            // This code is run on a separate thread.
            println!(
                "[client] ip = {}, thread = {:?}",
                ip,
                thread::current().id()
            );

            server_addr_rx
                .map_err(|e| panic!("failed to get server addr: {}", e))
                .and_then(|server_addr| {
                    println!("[client] Got server addr: {}", server_addr);

                    TcpStream::connect(&server_addr)
                        .map_err(|e| panic!("error connecting: {}", e))
                        .and_then(|_stream| {
                            println!("[client] Connected to server");
                            Ok(())
                        })
                })
        });
        let client = node::ipv4::nat(Ipv4NatBuilder::default(), client);

        let router = node::ipv4::router((server, client));
        let (spawn_complete, _ipv4_plug) = spawn::ipv4_tree(&handle, Ipv4Range::global(), router);

        spawn_complete.map(|((), ())| ())
    }));
    unwrap!(res)
}
