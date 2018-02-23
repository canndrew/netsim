mod tap;
mod tun;
mod build;

pub use self::tap::{EtherIfaceBuilder, EtherIface, UnboundEtherIface};
pub use self::tun::{Ipv4IfaceBuilder, Ipv4Iface, UnboundIpv4Iface};
pub use self::build::IfaceBuildError;

