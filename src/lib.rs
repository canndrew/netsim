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
//! let tap = TapBuilder::new();
//! tap.name("netsim");
//! tap.address(ipv4!("192.168.0.23"));
//! tap.netmask(ipv4!("255.255.255.0"));
//! tap.route(RouteV4::new(SubnetV4::new(ipv4!("0.0.0.0), 0), Some(ipv4!("192.168.0.1"))));
//! tap.build(&handle);
//!
//! // Read the first `EtherFrame` sent from the interface.
//! let frame = core.run({
//!     tap
//!     .into_future()
//!     .and_then(|(frame, _)| frame.unwrap())
//! }).unwrap();
//! ```
//!
//! # Higher-level APIs
//!
//! The other functions in the `spawn` module allow you to combine the steps above and more into a
//! single call. For example, you can spawn a thread into an environment with a single NIC, a local
//! IP address behind a NAT.
//!
//! ```ignore
//! extern crate netsim;
//! let (join_handle, gateway) = netsim::spawn::behind_gateway(|| {
//!     // packets sent here will appear NATed on `gateway`
//! });
//! ```

#![recursion_limit="128"]

extern crate libc;
#[macro_use]
extern crate unwrap;
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
#[cfg(test)]
extern crate bincode;
#[cfg(test)]
extern crate capabilities;
extern crate smoltcp;

/// Convert a variable-length slice to a fixed-length array
macro_rules! assert_len {
    ($len:tt, $slice:expr) => {{
        use std::ptr;

        union MaybeUninit<T: Copy> {
            init: T,
            uninit: (),
        }
        
        let mut array: MaybeUninit<[_; $len]> = MaybeUninit { uninit: () };
        let slice: &[_] = $slice;
        for i in 0..$len {
            let x = slice[i];
            unsafe {
                ptr::write(&mut array.init[i], x)
            }
        }

        unsafe {
            array.init
        }
    }}
}

//mod machine;
//pub mod plug;
pub mod spawn;
pub mod tap;
mod fd;
mod sys;
mod route;
mod ethernet;
mod ipv4_addr;
//mod ipv6;
//mod frame_buffer;
mod gateway;
//mod link;
mod util;
mod time;
mod subnet;
mod icmpv6;
mod hub;
mod veth;
mod veth_adaptor;
mod flush;
mod with_disconnect;
mod latency;

mod prelude;
mod priv_prelude;

pub use prelude::*;

