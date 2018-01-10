//! A library for network simulation and testing. *Currently linux-only*.
//!
//! This crate provides tools for simulating networks. It's target use-case is for automated
//! testing in internet-based applications. This documentation gives a brief rundown of the main
//! features.
//!
//! # Spawning threads into isolated network namespaces
//!
//! Network namespaces are a linux feature which can provide a thread or process with their own
//! view of the system's network interfaces and routing table.  This crate's `spawn` module
//! provides functions for spawning threads into their own network namespaces. The most primitive
//! of these functions is `new_namespace`, which is demonstrated below. In this example we list the
//! visible network interfaces using the [`get_if_addrs`](https://crates.io/crates/get_if_addrs)
//! library.
//!
//! ```ignore
//! extern crate netsim;
//! extern crate get_if_addrs;
//! use netsim::spawn;
//!
//! // First, check that there is more than one network interface. This will generally be true
//! // since there will at least be the loopback interface.
//! let interfaces = get_if_addrs::get_if_addrs().unwrap();
//! assert!(!interfaces.is_empty());
//!
//! // Now check how many network interfaces we can see inside a fresh network namespace. There
//! // should not be any.
//! let join_handle = spawn::new_namespace(|| {
//!     get_if_addrs::get_if_addrs().unwrap()
//! });
//! let interfaces = join_handle.join().unwrap();
//! assert!(interfaces.is_empty());
//! ```
//!
//! # Creating virtual interfaces
//!
//! We can create virtual (TAP) interfaces using the `Tap` type. A `Tap` is a
//! [`futures`](https://crates.io/crates/futures) `Stream + Sink` which can be used to read/write
//! raw ethernet frames to the interface. Here's an example using
//! [`tokio`](https://crates.io/crates/tokio-core).
//!
//! ```ignore
//! extern crate netsim;
//! #[macro_use]
//! extern crate net_literals;
//! extern crate tokio_core;
//! extern crate futures;
//! use netsim::tap::{TapBuilder, Tap, IfaceAddrV4};
//!
//! let core = Core::new().unwrap();
//! let handle = core.handle();
//!
//! // Create a network interface named "netsim"
//! let tap = {
//!     TapBuilder::new()
//!     .name("netsim")
//!     .address(IfaceAddrV4 {
//!         netmask: ipv4!("255.255.255.0"),
//!         address: ipv4!("192.168.0.23"),
//!     })
//!     .build(&handle)
//! };
//!
//! // Read the first `EtherFrame` sent out the interface.
//! let frame = core.run({
//!     tap
//!     .into_future()
//!     .and_then(|(frame, _)| frame.unwrap())
//! }).unwrap();
//! ```
//!
//! # Configure routes
//!
//! TODO
//!
//! # Higher-level APIs
//!
//! TODO
//!

//#![deny(missing_docs)]

extern crate libc;
#[macro_use]
extern crate unwrap;
extern crate tun;
//#[cfg(test)]
extern crate get_if_addrs;
//#[macro_use]
//extern crate lazy_static;
extern crate future_utils;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate void;
#[macro_use]
extern crate net_literals;
extern crate rand;
#[macro_use]
extern crate rand_derive;
#[macro_use]
extern crate ioctl_sys;
extern crate bytes;
extern crate mio;
#[macro_use]
extern crate quick_error;
#[macro_use]
extern crate log;
extern crate env_logger;

//mod machine;
//pub mod plug;
pub mod spawn;
pub mod tap;
mod fd;
mod sys;
mod route;
mod ethernet;
mod ipv4;
mod ipv6;
mod udp;
//mod frame_buffer;
mod gateway;
mod link;
mod util;
mod time;
mod subnet;
mod arp;
mod icmpv6;

mod prelude;
mod priv_prelude;

pub use prelude::*;

