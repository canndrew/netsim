//! Contains utilities for creating virtual TUN/TAP network interfaces.

mod tap;
mod tun;
mod build;
mod config;

pub use self::tap::{EtherIfaceBuilder, EtherIface, UnboundEtherIface};
pub use self::tun::{IpIfaceBuilder, IpIface, UnboundIpIface};
pub use self::build::IfaceBuildError;
pub use self::config::*;

