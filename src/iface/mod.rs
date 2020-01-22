//! Contains utilities for creating virtual TUN/TAP network interfaces.

mod build;
mod config;
mod tap;
mod tun;

pub use self::build::IfaceBuildError;
pub use self::config::*;
pub use self::tap::{EtherIface, EtherIfaceBuilder, UnboundEtherIface};
pub use self::tun::{IpIface, IpIfaceBuilder, UnboundIpIface};
