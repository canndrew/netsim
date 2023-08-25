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

pub use {
    machine::Machine,
    iface::stream::IpIface,
    connect::{connect, Connect},
    network::Ipv4Network,
    stream_ext::PacketStreamExt,
};

//mod packet2;

#[cfg(test)]
mod tests;

