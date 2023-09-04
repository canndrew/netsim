#![cfg_attr(feature="cargo-clippy", allow(clippy::let_unit_value))]

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
    stream_ext::SinkStreamExt,
};

#[cfg(test)]
mod tests;

