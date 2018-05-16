//! `netsim` is a crate for simulating networks for the sake of testing network-oriented Rust
//! code. You can use it to run Rust functions in network-isolated containers, and assemble
//! virtual networks for these functions to communicate over.
//!
//! # Spawning threads into isolated network namespaces
//!
//! Network namespaces are a linux feature which can provide a thread or process with its own view
//! of the system's network interfaces and routing table. This crate's `spawn` module provides
//! functions for spawning threads into their own network namespaces. The most primitive of these
//! functions is `new_namespace`, which is demonstrated below. In this example we list the visible
//! network interfaces using the [`get_if_addrs`](https://crates.io/crates/get_if_addrs) crate.
//!
//! ```rust
//! extern crate netsim;
//! extern crate get_if_addrs;
//! extern crate tokio_core;
//! use netsim::spawn;
//! use tokio_core::reactor::Core;
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
//! let mut core = Core::new().unwrap();
//! let interfaces = core.run(spawn_complete).unwrap();
//! assert!(interfaces.is_empty());
//! ```
//!
//! This demonstrates how to launch a thread - perhaps running an automated test - into a clean
//! environment. However an environment with no network interfaces is pretty useless...
//!
//! # Creating virtual interfaces
//!
//! We can create virtual IP and Ethernet interfaces using the types in the `iface` module. For
//! example, `Ipv4Iface` lets you create a new IP (TUN) interface and implements `futures::{Stream,
//! Sink}` so that you can read/write raw packets to it.
//!
//! ```rust,no_run
//! extern crate netsim;
//! extern crate tokio_core;
//! extern crate futures;
//!
//! use std::net::Ipv4Addr;
//! use tokio_core::reactor::Core;
//! use futures::{Future, Stream};
//! use netsim::iface::Ipv4IfaceBuilder;
//! 
//! let mut core = Core::new().unwrap();
//! let handle = core.handle();
//! 
//! // Create a network interface named "netsim"
//! let iface = {
//!     Ipv4IfaceBuilder::new()
//!     .name("netsim")
//!     .address(Ipv4Addr::new(192, 168, 0, 23))
//!     .netmask(Ipv4Addr::new(255, 255, 255, 0))
//!     .build(&handle)
//!     .unwrap()
//! };
//! 
//! // Read the first `Ipv4Packet` sent from the interface.
//! let packet = core.run({
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
//! Rather than performing the above two steps individually, you can use the functions in the
//! `spawn` module to set up various network environments for you. For example,
//! `spawn::on_subnet_v4` will spawn a thread with a single network interface configured to use the
//! given subnet. It returns a `JoinHandle` to join the thread with and an `Ipv4Plug` to read/write
//! packets to the thread's network interface.
//!
//! ```rust
//! extern crate netsim;
//! extern crate tokio_core;
//! extern crate futures;
//!
//! use std::net::UdpSocket;
//! use tokio_core::reactor::Core;
//! use futures::{Future, Stream};
//! use netsim::{spawn, Ipv4Range};
//! use netsim::wire::Ipv4Payload;
//!
//! let mut core = Core::new().unwrap();
//! let handle = core.handle();
//!
//! let subnet = Ipv4Range::local_subnet_10();
//! let (spawn_complete, plug) = spawn::on_subnet_v4(&handle, subnet, |ip_addr| {
//!     let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
//!     socket.send_to(b"hello world", "10.1.2.3:4567").unwrap();
//! });
//!
//! core.run({
//!     plug.rx
//!     .into_future()
//!     .map(|(packet_opt, _)| {
//!         let packet = packet_opt.unwrap();
//!         match packet.payload() {
//!             Ipv4Payload::Udp(udp) => {
//!                 assert_eq!(&udp.payload()[..], &b"hello world"[..]);
//!             },
//!             _ => panic!(),
//!         }
//!     })
//! }).unwrap()
//! ```
//!
//! # Simulating networks of communicating nodes
//!
//! To simulate a bunch of IPv4-connected nodes you can use the functions in the `node` module
//! along with the `spawn::network_v4` function to describe and launch a simluated network test.
//!
//! ```rust
//! extern crate tokio_core;
//! extern crate future_utils;
//! extern crate netsim;
//! 
//! use std::net::UdpSocket;
//! use tokio_core::reactor::Core;
//! use netsim::{spawn, node, Ipv4Range};
//! 
//! let mut core = Core::new().unwrap();
//! let handle = core.handle();
//!
//! let (tx, rx) = std::sync::mpsc::channel();
//! let node_a = node::endpoint_v4(move |ip_addr| {
//!     let socket = UdpSocket::bind(("0.0.0.0", 1234)).unwrap();
//!     tx.send(ip_addr).unwrap();
//!     let mut buffer = [0; 1024];
//!     let (n, addr) = socket.recv_from(&mut buffer).unwrap();
//!     buffer[..n].to_owned()
//! });
//!
//! let node_b = node::endpoint_v4(move |_ip_addr| {
//!     let ip = rx.recv().unwrap();
//!     let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
//!     socket.send_to(b"hello world", (ip, 1234)).unwrap();
//! });
//!
//! let router_node = node::router_v4((node_a, node_b));
//! let (spawn_complete, _plug) = spawn::network_v4(&handle, Ipv4Range::global(), router_node);
//! let (received, ()) = core.run(spawn_complete).unwrap();
//! assert_eq!(&received[..], b"hello world");
//! ```
//!
//! Note that we need to make sure to drive the `Core` while blocking on the `JoinHandle` in a
//! separate thread. A future version of this library may clean this situation up.
//!
//! # All the rest
//!
//! It's possible to set up more complicated (non-hierarchical) network topologies, ethernet
//! networks, namespaces with multiple interfaces etc. by directly using the primitives in this
//! library. Have an explore of the API, and if anything needs clarification or could be designed
//! better then drop a message on the bug tracker :)

#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#![deny(missing_docs)]
#![cfg_attr(feature="clippy", allow(redundant_field_names))]
#![cfg_attr(feature="clippy", allow(single_match))]
#![cfg_attr(feature="clippy", allow(match_same_arms))]
#![cfg_attr(feature="clippy", allow(decimal_literal_representation))]

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
extern crate tokio_io;
extern crate tokio_core;
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
pub mod iface;
pub mod node;
pub mod device;
pub mod wire;
pub mod spawn;
#[cfg(test)]
mod test;

pub use range::{Ipv4Range, Ipv6Range, IpRangeParseError};
pub use route::{RouteV4, RouteV6, AddRouteError};
pub use spawn_complete::SpawnComplete;
pub use pcap::Ipv4Log;

