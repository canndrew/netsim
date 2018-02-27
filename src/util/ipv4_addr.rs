use priv_prelude::*;
use rand;

pub trait Ipv4AddrExt {
    /// Get a random, global IPv4 address.
    fn random_global() -> Ipv4Addr;
    /// Returns `true` if this is a global IPv4 address
    fn is_global(&self) -> bool;
    /// Returns `true` if this is a reserved IPv4 address.
    fn is_reserved(&self) -> bool;
}

impl Ipv4AddrExt for Ipv4Addr {
    fn random_global() -> Ipv4Addr {
        loop {
            let x: u32 = rand::random();
            let ip = Ipv4Addr::from(x);
            if Ipv4AddrExt::is_global(&ip) {
                return ip;
            }
        }
    }

    fn is_global(&self) -> bool {
        !(  self.is_loopback()
        ||  self.is_private()
        ||  self.is_link_local()
        ||  self.is_multicast()
        ||  self.is_broadcast()
        ||  self.is_documentation()
        ||  self.is_reserved()
        )
    }

    fn is_reserved(&self) -> bool {
        u32::from(*self) & 0xf000_0000 == 0xf000_0000
    }
}

