//! This is "hello world" of netsim.
//! This example demonstrates how to create a virtual network with one node and send some data
//! to it over UDP.

extern crate bytes;
extern crate futures;
extern crate netsim;
extern crate tokio;
#[macro_use]
extern crate unwrap;

use bytes::Bytes;
use futures::sync::oneshot;
use futures::{future, Future};
use netsim::wire::{Ipv4Fields, Ipv4Packet, Ipv4PayloadFields, UdpFields};
use netsim::{node, spawn, Ipv4Range, Network};
use tokio::net::UdpSocket;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str;

fn main() {
    let network = Network::new();
    let handle = network.handle();

    let mut runtime = unwrap!(tokio::runtime::Runtime::new());
    let res = runtime.block_on(future::lazy(move || {
        let (server_addr_tx, server_addr_rx) = oneshot::channel();
        let server_node = node::ipv4::machine(|ip| {
            println!("[server] ip = {}", ip);

            let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
            let socket = unwrap!(UdpSocket::bind(&bind_addr));
            // Send server's address to client
            unwrap!(server_addr_tx.send(unwrap!(socket.local_addr())));

            socket
                .recv_dgram(vec![0u8; 1024])
                .map_err(|e| panic!("error receiving: {}", e))
                .map(|(_socket, buf, len, addr)| {
                    let s = unwrap!(str::from_utf8(&buf[..len]));
                    println!("[server] received: {}, from: {}", s, addr);
                })
        });
        // Build and run server node on simulated network
        let (spawn_complete, ipv4_plug) =
            spawn::ipv4_tree(&handle, Ipv4Range::global(), server_node);

        let (packet_tx, _packet_rx) = ipv4_plug.split();

        server_addr_rx
            .map_err(|e| panic!("failed to receive server addr: {}", e))
            .and_then(move |server_addr| {
                let server_addr = match server_addr {
                    SocketAddr::V4(addr) => addr,
                    _ => panic!("Ipv6 was not expected: {}", server_addr),
                };

                // Construct UDP datagram to send to our server node
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
                packet_tx.unbounded_send(datagram);

                // Wait till server node is finished
                spawn_complete
            })
    }));
    unwrap!(res)
}
