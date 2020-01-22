use crate::priv_prelude::*;
use rand;

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub enum Ipv6AddrClass {
    Unspecified,
    Loopback,
    Ipv4Mapped,
    Ipv4To6,
    Discard,
    Reserved,
    Documentation,
    Ipv6To4,
    UniqueLocal,
    LinkLocal,
    Multicast,
    Global,
}

/// Extension methods for IPv6 addresses
pub trait Ipv6AddrExt {
    /// Get a random, global IPv6 address.
    fn random_global() -> Ipv6Addr;
    /// Check if this is a unicast global address
    fn is_unicast_global(&self) -> bool;
    /// Check if this is a unicast link local address
    fn is_unicast_link_local(&self) -> bool;
    /// Check if this is a unicast site local address
    fn is_unicast_site_local(&self) -> bool;
    /// Check if this is a unique local address
    fn is_unique_local(&self) -> bool;
    /// Check if this is a documentation address
    fn is_documentation(&self) -> bool;
    /// Clasify the address.
    fn class(&self) -> Ipv6AddrClass;
    /// Create an `Ipv6Addr` representing a netmask
    fn from_netmask_bits(bits: u8) -> Ipv6Addr;
}

impl Ipv6AddrExt for Ipv6Addr {
    /// Get a random, global IPv6 address.
    fn random_global() -> Ipv6Addr {
        let x0 = rand::random::<u64>();
        let x1 = rand::random::<u64>();
        let mut x = (u128::from(x0) << 64) | u128::from(x1);
        loop {
            let ip = Ipv6Addr::from(x);
            if Ipv6AddrExt::is_unicast_global(&ip) {
                return ip;
            }
            x >>= 8;
            x |= u128::from(rand::random::<u8>()) << 120;
        }
    }

    fn is_unicast_global(&self) -> bool {
        !(self.is_loopback()
            || Ipv6AddrExt::is_unicast_link_local(self)
            || Ipv6AddrExt::is_unicast_site_local(self)
            || Ipv6AddrExt::is_unique_local(self)
            || self.is_unspecified()
            || Ipv6AddrExt::is_documentation(self)
            || self.is_multicast())
    }

    fn is_unicast_link_local(&self) -> bool {
        let range = Ipv6Range::new(ipv6!("fe80::"), 10);
        range.contains(*self)
    }

    fn is_unicast_site_local(&self) -> bool {
        let range = Ipv6Range::new(ipv6!("fec0::"), 10);
        range.contains(*self)
    }

    fn is_unique_local(&self) -> bool {
        let range = Ipv6Range::new(ipv6!("fc00::"), 7);
        range.contains(*self)
    }

    fn is_documentation(&self) -> bool {
        let range = Ipv6Range::new(ipv6!("2001:0db8::"), 32);
        range.contains(*self)
    }

    fn class(&self) -> Ipv6AddrClass {
        if *self == ipv6!("::") {
            return Ipv6AddrClass::Unspecified;
        }
        if *self == ipv6!("::1") {
            return Ipv6AddrClass::Loopback;
        }
        if Ipv6Range::new(ipv6!("::ffff:0:0"), 96).contains(*self) {
            return Ipv6AddrClass::Ipv4Mapped;
        }
        if Ipv6Range::new(ipv6!("64:ff9b::"), 96).contains(*self) {
            return Ipv6AddrClass::Ipv4To6;
        }
        if Ipv6Range::new(ipv6!("100::"), 64).contains(*self) {
            return Ipv6AddrClass::Discard;
        }
        if Ipv6Range::new(ipv6!("2001::"), 23).contains(*self) {
            return Ipv6AddrClass::Reserved;
        }
        if Ipv6Range::new(ipv6!("2001:db8::"), 32).contains(*self) {
            return Ipv6AddrClass::Documentation;
        }
        if Ipv6Range::new(ipv6!("2002::"), 16).contains(*self) {
            return Ipv6AddrClass::Ipv6To4;
        }
        if Ipv6Range::new(ipv6!("fc00::"), 7).contains(*self) {
            return Ipv6AddrClass::UniqueLocal;
        }
        if Ipv6Range::new(ipv6!("fe80::"), 10).contains(*self) {
            return Ipv6AddrClass::LinkLocal;
        }
        if Ipv6Range::new(ipv6!("ff00::"), 8).contains(*self) {
            return Ipv6AddrClass::Multicast;
        }
        Ipv6AddrClass::Global
    }

    fn from_netmask_bits(bits: u8) -> Ipv6Addr {
        Ipv6Addr::from(!((!0u128) >> bits))
    }
}
