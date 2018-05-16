use priv_prelude::*;
use rand;

pub trait Ipv6AddrExt {
    /// Get a random, global IPv6 address.
    fn random_global() -> Ipv6Addr;
    fn is_unicast_global(&self) -> bool;
    fn is_unicast_link_local(&self) -> bool;
    fn is_unicast_site_local(&self) -> bool;
    fn is_unique_local(&self) -> bool;
    fn is_documentation(&self) -> bool;
    /// Create an `Ipv6Addr` representing a netmask
    fn from_netmask_bits(bits: u8) -> Ipv6Addr;
}

impl Ipv6AddrExt for Ipv6Addr {
    /// Get a random, global IPv6 address.
    fn random_global() -> Ipv6Addr {
        let x0 = rand::random::<u64>();
        let x1 = rand::random::<u64>();
        let mut x = ((x0 as u128) << 64) | (x1 as u128);
        loop {
            let ip = Ipv6Addr::from(x);
            if Ipv6AddrExt::is_unicast_global(&ip) {
                return ip
            }
            x >>= 8;
            x |= (rand::random::<u8>() as u128) << 120;
        }
    }

    fn is_unicast_global(&self) -> bool {
        !(
            self.is_loopback() ||
            Ipv6AddrExt::is_unicast_link_local(self) ||
            Ipv6AddrExt::is_unicast_site_local(self) ||
            Ipv6AddrExt::is_unique_local(self) ||
            self.is_unspecified() ||
            Ipv6AddrExt::is_documentation(self) ||
            self.is_multicast()
        )
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

    fn from_netmask_bits(bits: u8) -> Ipv6Addr {
        Ipv6Addr::from(!((!0u128) >> bits))
    }
}


