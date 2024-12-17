#![allow(clippy::let_unit_value)]

extern crate self as netsim;

mod priv_prelude;
mod namespace;
mod machine;
mod iface;
mod ioctl;
mod network;
mod connect;
mod stream_ext;
pub mod adapter;
pub mod device;
pub mod packet;
mod sys;

pub use {
    machine::{Machine, JoinHandle},
    iface::{
        create::IpIfaceBuilder,
        stream::{IpIface, IpSinkStream},
    },
    connect::connect,
    network::{Ipv4Network, Ipv6Network, NetworkParseError, Ipv4NetworkIter, Ipv6NetworkIter},
    netsim_macros::{ipv4_network, ipv6_network, isolate},
    stream_ext::SinkStreamExt,
    tokio,
};

#[cfg(test)]
mod tests;

