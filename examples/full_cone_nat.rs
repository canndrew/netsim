extern crate env_logger;
extern crate futures;
extern crate netsim;
extern crate tokio;
#[macro_use]
extern crate unwrap;
#[macro_use]
extern crate net_literals;

use futures::sync::oneshot;
use futures::{future, Future};
use netsim::device::ipv4::Ipv4NatBuilder;
use netsim::{node, spawn, Ipv4Range, Network};
use tokio::net::UdpSocket;
use tokio::runtime::Runtime;

use std::net::{SocketAddr, SocketAddrV4};
use std::str;

fn main() {
    let network = Network::new();
    let handle = network.handle();

    let mut runtime = unwrap!(Runtime::new());
    let res = runtime.block_on(future::lazy(move || {
        let (server_addr_tx, server_addr_rx) = oneshot::channel();
        let (client0_addr_tx, client0_addr_rx) = oneshot::channel();
        let server_node = node::ipv4::machine(|ip| {
            println!("[server] ip = {}", ip);

            let bind_addr = SocketAddr::V4(SocketAddrV4::new(ip, 0));
            let socket = unwrap!(UdpSocket::bind(&bind_addr));
            unwrap!(server_addr_tx.send(unwrap!(socket.local_addr())));

            socket
                .recv_dgram(vec![0u8; 1024])
                .map_err(|e| panic!("error sending: {}", e))
                .map(|(_socket, buf, len, addr)| {
                    let s = unwrap!(str::from_utf8(&buf[..len]));
                    println!("[server] received: {}, from: {}", s, addr);
                    unwrap!(client0_addr_tx.send(addr));
                })
        });

        let client0_node = node::ipv4::machine(|ip| {
            println!("[client0] ip = {}", ip);

            server_addr_rx
                .map_err(|e| panic!("error receiving server addr: {}", e))
                .and_then(|server_addr| {
                    println!("[client0] Got server addr: {}", server_addr);

                    let socket = unwrap!(UdpSocket::bind(&addr!("0.0.0.0:0")));
                    socket
                        .send_dgram(b"hello world", &server_addr)
                        .map_err(|e| panic!("error sending: {}", e))
                        .and_then(|(socket, _buf)| {
                            socket
                                .recv_dgram(vec![0; 1024])
                                .map_err(|e| panic!("error receiving: {}", e))
                                .map(|(_socket, buf, len, addr)| {
                                    let s = unwrap!(str::from_utf8(&buf[..len]));
                                    println!("[client0] received: {}, from: {}", s, addr);
                                })
                        })
                })
        });

        let nat_builder = Ipv4NatBuilder::new().subnet(Ipv4Range::local_subnet_192(0));
        let client0_behind_nat_node = node::ipv4::nat(nat_builder, client0_node);

        let client1_node = node::ipv4::machine(|ip| {
            println!("[client1] ip = {}", ip);

            client0_addr_rx
                .map_err(|e| panic!("failed to receive client0 addr: {}", e))
                .and_then(|client0_addr| {
                    println!("[client1] Got client0 addr: {}", client0_addr);

                    let socket = unwrap!(UdpSocket::bind(&addr!("0.0.0.0:0")));
                    socket
                        .send_dgram(b"this is client1!", &client0_addr)
                        .map_err(|e| panic!("error sending: {}", e))
                        .map(|(_socket, _buf)| ())
                })
        });

        let router_node = node::ipv4::router((server_node, client0_behind_nat_node, client1_node));
        let (spawn_complete, _ipv4_plug) =
            spawn::ipv4_tree(&handle, Ipv4Range::global(), router_node);

        spawn_complete.map(|((), (), ())| ())
    }));
    unwrap!(res)
}
