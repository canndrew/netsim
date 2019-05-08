//! `netsim` is a crate for simulating networks for the sake of testing network-oriented Rust
//! code. You can use it to run Rust functions in network-isolated containers, and assemble
//! virtual networks for these functions to communicate over.
//!
//! # Spawning threads into isolated network namespaces
//!
//! Network namespaces are a linux feature which can provide a thread or process with its own view
//! of the system's network interfaces and routing table. This crate's `spawn` module provides the
//! `new_namespace` function for spawning threads into their own network namespaces.  In this
//! demonstration we list the visible network interfaces using the
//! [`get_if_addrs`](https://crates.io/crates/get_if_addrs) crate.
//!
//! ```rust
//! extern crate netsim;
//! extern crate get_if_addrs;
//! extern crate tokio;
//! use netsim::spawn;
//! use tokio::runtime::Runtime;
//! use get_if_addrs::get_if_addrs;
//!
//! // First, check that there is more than one network interface. This will generally be true
//! // since there will at least be the loopback interface.
//! let interfaces = get_if_addrs().unwrap();
//! assert!(interfaces.len() > 0);
//!
//! // Now check how many network interfaces we can see inside a fresh network namespace. There
//! // should be zero.
//! let spawn_complete = spawn::new_namespace(|| {
//!     get_if_addrs().unwrap()
//! });
//! let mut runtime = Runtime::new().unwrap();
//! let interfaces = runtime.block_on(spawn_complete).unwrap();
//! assert!(interfaces.is_empty());
//! ```
//!
//! This demonstrates how to launch a thread - perhaps running an automated test - into a sandboxed
//! environment. However an environment with no network interfaces is pretty useless...
//!
//! # Creating virtual interfaces
//!
//! We can create virtual IP and Ethernet interfaces using the types in the `iface` module. For
//! example, `IpIface` lets you create a new IP (TUN) interface and implements `futures::{Stream,
//! Sink}` so that you can read/write raw packets to it.
//!
//! ```rust,should_panic
//! extern crate netsim;
//! extern crate tokio;
//! extern crate futures;
//!
//! use std::net::Ipv4Addr;
//! use tokio::runtime::Runtime;
//! use futures::{Future, Stream};
//! use netsim::iface::IpIfaceBuilder;
//! use netsim::spawn;
//!
//! let mut runtime = Runtime::new().unwrap();
//!
//! // Create a network interface named "netsim"
//! // Note: This will likely fail with "permission denied" unless we run it in a fresh network
//! // environment
//! let iface = {
//!     IpIfaceBuilder::new()
//!     .name("netsim")
//!     .ipv4_addr(Ipv4Addr::new(192, 168, 0, 24), 24)
//!     .build()
//!     .unwrap()
//! };
//!
//! // Read the first `Ipv4Packet` sent from the interface.
//! let packet = runtime.block_on({
//!     iface
//!     .into_future()
//!     .map_err(|(e, _)| e)
//!     .map(|(packet_opt, _)| packet_opt.unwrap())
//! }).unwrap();
//! ```
//!
//! However, for simply testing network code, you don't need to create interfaces manually like
//! this.
//!
//! # Sandboxing network code
//!
//! Rather than performing the above two steps individually, you can use the `spawn::ipv4_tree`
//! function along with the `node` module to set up a namespace with an IPv4 interface for you.
//!
//! ```rust
//! extern crate netsim;
//! extern crate tokio;
//! extern crate futures;
//! extern crate void;
//!
//! use tokio::runtime::Runtime;
//! use tokio::net::UdpSocket;
//! use futures::{Future, Stream};
//! use netsim::{spawn, node, Network, Ipv4Range};
//! use netsim::wire::Ipv4Payload;
//! use void::{self, ResultVoidExt};
//!
//! // Create an event loop and a network to bind devices to.
//! let mut runtime = Runtime::new().unwrap();
//! let network = Network::new();
//! let handle = network.handle();
//!
//! runtime.block_on(futures::future::lazy(move || {
//!     // Spawn a network with a single node - a machine with an IPv4 interface in the 10.0.0.0/8
//!     // range, running the given callback.
//!     let (spawn_complete, ipv4_plug) = spawn::ipv4_tree(
//!         &handle,
//!         Ipv4Range::local_subnet_10(),
//!         node::ipv4::machine(|ipv4_addr| {
//!             // Send a packet out the interface
//!             let bind_addr = "0.0.0.0:0".parse().unwrap();
//!             let socket = UdpSocket::bind(&bind_addr).unwrap();
//!             let addr = "10.1.2.3:4567".parse().unwrap();
//!             socket
//!             .send_dgram(b"hello world", &addr)
//!             .map_err(|e| panic!("error sending: {}", e))
//!             .map(|(_socket, _buffer)| ())
//!         }),
//!     );
//!
//!     let (packet_tx, packet_rx) = ipv4_plug.split();
//!
//!     // Inspect the packet sent out the interface.
//!     packet_rx
//!     .into_future()
//!     .map_err(|(v, _)| v)
//!     .map(|(packet_opt, _)| {
//!         let packet = packet_opt.unwrap();
//!         match packet.payload() {
//!             Ipv4Payload::Udp(udp) => {
//!                 assert_eq!(&udp.payload()[..], &b"hello world"[..]);
//!             },
//!             _ => panic!("unexpected payload"),
//!         }
//!     })
//! })).void_unwrap();
//! ```
//!
//! # Simulating networks of communicating nodes
//!
//! Using the `spawn` and `node` modules you can set up a bunch of nodes connected over a virtual
//! network.
//!
//! ```rust
//! extern crate tokio;
//! extern crate futures;
//! extern crate future_utils;
//! extern crate netsim;
//!
//! use std::net::{SocketAddr, IpAddr};
//! use tokio::runtime::Runtime;
//! use tokio::net::UdpSocket;
//! use futures::Future;
//! use netsim::{spawn, node, Network, Ipv4Range};
//!
//! // Create an event loop and a network to bind devices to.
//! let mut runtime = Runtime::new().unwrap();
//! let network = Network::new();
//! let handle = network.handle();
//!
//! // Drive the network on the event loop and get the data returned by the receiving node.
//! let (received, ()) = runtime.block_on(futures::future::lazy(move || {
//!     let (tx, rx) = std::sync::mpsc::channel();
//!
//!     // Create a machine which will receive a UDP packet and return its contents
//!     let receiver_node = node::ipv4::machine(move |ipv4_addr| {
//!         let bind_addr = "0.0.0.0:1234".parse().unwrap();
//!         let socket = UdpSocket::bind(&bind_addr).unwrap();
//!         /// Tell the sending node our IP address
//!         tx.send(ipv4_addr).unwrap();
//!         let mut buffer = [0; 1024];
//!
//!         socket
//!         .recv_dgram(vec![0; 1024])
//!         .map_err(|e| panic!("error receiving: {}", e))
//!         .map(|(_socket, mut buffer, len, _addr)| {
//!             buffer.truncate(len);
//!             buffer
//!         })
//!     });
//!
//!     // Create the machine which will send the UDP packet
//!     let sender_node = node::ipv4::machine(move |_ipv4_addr| {
//!         let receiver_ip = rx.recv().unwrap();
//!         let bind_addr = "0.0.0.0:0".parse().unwrap();
//!         let socket = UdpSocket::bind(&bind_addr).unwrap();
//!         socket
//!         .send_dgram(b"hello world", &SocketAddr::new(IpAddr::V4(receiver_ip), 1234))
//!         .map_err(|e| panic!("error sending: {}", e))
//!         .map(|(_socket, _buffer)| ())
//!     });
//!
//!     // Connect the sending and receiving nodes via a router
//!     let router_node = node::ipv4::router((receiver_node, sender_node));
//!
//!     // Run the network with the router as the top-most node. `_plug` could be used send/receive
//!     // packets from/to outside the network
//!     let (spawn_complete, _plug) = spawn::ipv4_tree(&handle, Ipv4Range::global(), router_node);
//!
//!     spawn_complete
//! })).unwrap();
//! assert_eq!(&received[..], b"hello world");
//! ```
//!
//! # All the rest
//!
//! It's possible to set up more complicated (non-hierarchical) network topologies, ethernet
//! networks, namespaces with multiple interfaces etc. by directly using the primitives in the
//! `device` module. Have an explore of the API, and if anything needs clarification or could be
//! better designed then let us know on the bug tracker :)

#![deny(missing_docs)]
#![cfg_attr(feature="cargo-clippy", allow(redundant_field_names))]
#![cfg_attr(feature="cargo-clippy", allow(single_match))]
#![cfg_attr(feature="cargo-clippy", allow(match_same_arms))]
#![cfg_attr(feature="cargo-clippy", allow(decimal_literal_representation))]

extern crate libc;
extern crate rand;
extern crate byteorder;
extern crate bytes;
#[macro_use]
extern crate unwrap;
extern crate void;
extern crate get_if_addrs;
#[macro_use]
extern crate net_literals;
#[macro_use]
extern crate quick_error;
#[macro_use]
extern crate ioctl_sys;
#[macro_use]
extern crate log;
extern crate mio;
extern crate futures;
extern crate tokio;
#[macro_use]
extern crate rand_derive;
extern crate future_utils;

#[cfg(test)]
extern crate capabilities;
#[cfg(test)]
extern crate env_logger;
#[cfg(test)]
extern crate statrs;

/// Convert a variable-length slice to a fixed-length array
macro_rules! slice_assert_len {
    ($len:tt, $slice:expr) => {{

        use std::ptr;

        union MaybeUninit<T: Copy> {
            init: T,
            uninit: (),
        }

        assert_eq!($slice.len(), $len);
        let mut array: MaybeUninit<[_; $len]> = MaybeUninit { uninit: () };
        let slice: &[_] = $slice;
        for (i, x) in slice.iter().enumerate() {
            unsafe {
                ptr::write(&mut array.init[i], *x)
            }
        }

        unsafe {
            array.init
        }
    }}
}

mod priv_prelude;
mod util;
mod sys;
mod ioctl;
mod async_fd;
mod route;
mod range;
mod spawn_complete;
mod process_handle;
mod pcap;
mod plug;
mod network;
pub mod iface;
pub mod node;
pub mod device;
pub mod wire;
pub mod spawn;
#[cfg(test)]
mod test;

pub use crate::range::{Ipv4Range, Ipv6Range, IpRangeParseError};
pub use crate::route::{Ipv4Route, Ipv6Route, AddRouteError};
pub use crate::spawn_complete::SpawnComplete;
pub use crate::pcap::IpLog;
pub use crate::network::{Network, NetworkHandle};
pub use crate::util::{ipv4_addr::Ipv4AddrExt, ipv6_addr::Ipv6AddrExt};

