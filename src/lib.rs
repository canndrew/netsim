#![feature(slice_ptr_get)]

mod priv_prelude;
mod namespace;
mod machine;
mod iface;
mod ioctl;
mod network;
mod connect;
mod adapter;
mod stream_ext;
mod packet;

pub use {
    machine::Machine,
    iface::stream::IpIface,
    connect::{connect, Connect},
    packet::{IpPacket, Ipv4Packet, Ipv6Packet},
};

