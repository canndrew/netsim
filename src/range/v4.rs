use priv_prelude::*;
use super::*;
use rand;

/// A range of IPv4 addresses with a common prefix
#[derive(Clone, Copy)]
pub struct Ipv4Range {
    addr: Ipv4Addr,
    bits: u8,
}

impl fmt::Debug for Ipv4Range {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.addr, self.bits)
    }
}

impl Ipv4Range {
    /// Create an IPv4 range with the given base address and netmask prefix length.
    ///
    /// # Example
    ///
    /// Create the subnet 192.168.0.0/24 with `Ipv4Range::new(ipv4!("192.168.0.0"), 24)`
    pub fn new(addr: Ipv4Addr, bits: u8) -> Ipv4Range {
        let mask = !((!0u32).checked_shr(u32::from(bits)).unwrap_or(0));
        Ipv4Range {
            addr: Ipv4Addr::from(u32::from(addr) & mask),
            bits,
        }
    }

    /// Return the entire IPv4 range, eg. 0.0.0.0/0
    pub fn global() -> Ipv4Range {
        Ipv4Range {
            addr: ipv4!("0.0.0.0"),
            bits: 0,
        }
    }

    /// Returns the local network subnet 10.0.0.0/8
    pub fn local_subnet_10() -> Ipv4Range {
        Ipv4Range {
            addr: Ipv4Addr::new(10, 0, 0, 0),
            bits: 8,
        }
    }

    /// Returns a local network subnet 172.(16 | x).0.0/16 where x is a 4-bit number given by
    /// `block`
    ///
    /// # Panics
    ///
    /// If `block & 0xf0 != 0`
    pub fn local_subnet_172(block: u8) -> Ipv4Range {
        assert!(block < 16);
        Ipv4Range {
            addr: Ipv4Addr::new(172, 16 | block, 0, 0),
            bits: 16,
        }
    }

    /// Returns the local subnet 192.168.x.0/24 where x is given by `block`.
    pub fn local_subnet_192(block: u8) -> Ipv4Range {
        Ipv4Range {
            addr: Ipv4Addr::new(192, 168, block, 0),
            bits: 24,
        }
    }

    /// Returns a random local network subnet from one of the ranges 10.0.0.0, 172.16.0.0 or
    /// 192.168.0.0
    pub fn random_local_subnet() -> Ipv4Range {
        #[derive(Rand)]
        enum Subnet {
            S10,
            S172(u8),
            S192(u8),
        };

        match rand::random() {
            Subnet::S10 => Ipv4Range::local_subnet_10(),
            Subnet::S172(x) => Ipv4Range::local_subnet_172(x & 0x0f),
            Subnet::S192(x) => Ipv4Range::local_subnet_192(x),
        }
    }

    /// Get the netmask as an IP address
    pub fn netmask(&self) -> Ipv4Addr {
        Ipv4Addr::from(!((!0u32).checked_shr(u32::from(self.bits)).unwrap_or(0)))
    }

    /// Get the number of netmask prefix bits
    pub fn netmask_prefix_length(&self) -> u8 {
        self.bits
    }

    /// Get the base address of the range, ie. the lowest IP address which is part of the range.
    pub fn base_addr(&self) -> Ipv4Addr {
        self.addr
    }

    /// Get a default IP address for the range's gateway. This is one higher than the base address
    /// of the range. eg. for 10.0.0.0/8, the default address for the gateway will be 10.0.0.1
    pub fn gateway_ip(&self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.addr) | 1)
    }

    /// Get a random IP address from the range which is not the base address or the default
    /// for the gateway address.
    pub fn random_client_addr(&self) -> Ipv4Addr {
        let mask = !0 >> self.bits;
        assert!(mask > 1);
        let class = if self.bits == 0 {
            Ipv4AddrClass::Global
        } else {
            self.addr.class()
        };

        loop {
            let x = rand::random::<u32>() & mask;
            if x < 2 {
                continue
            }
            let addr = Ipv4Addr::from(u32::from(self.addr) | x);
            if class != addr.class() {
                continue
            }
            return addr;
        };
    }

    /// Check whether this range contains the given IP address
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        let base_addr = u32::from(self.addr);
        let test_addr = u32::from(ip);
        (base_addr ^ test_addr).leading_zeros() >= u32::from(self.bits)
    }

    /// Check if given address is a broadcast address for this IP range/subnet.
    pub fn is_broadcast(&self, ip: Ipv4Addr) -> bool {
        let host_broadcast = 0xFFFF_FFFF >> self.bits;
        let host_part = u32::from(ip) & host_broadcast;
        ip.is_broadcast() || (self.contains(ip) && host_part == host_broadcast)
    }

    /// Split a range into `num` sub-ranges
    ///
    /// # Panics
    ///
    /// If the range is too small to be split up that much.
    pub fn split(self, num: u32) -> Vec<Ipv4Range> {
        let mut ret = Vec::with_capacity(num as usize);
        let mut n = 0u32;
        let class = if self.bits == 0 {
            Ipv4AddrClass::Global
        } else {
            self.addr.class()
        };
        loop {
            let mut n_reversed = 0;
            for i in 0..32 {
                if n & (1 << i) != 0 {
                    n_reversed |= 0x8000_0000u32 >> i;
                }
            }
            let base_addr = u32::from(self.addr);
            let ip = base_addr | (n_reversed >> self.bits);
            let ip = Ipv4Addr::from(ip);
            if class != ip.class() {
                n += 1;
                continue;
            }
            ret.push(Ipv4Range {
                addr: ip,
                bits: 0,
            });
            if ret.len() == num as usize {
                break;
            }
            n += 1;
        }
        let extra_bits = (32 - n.leading_zeros()) as u8;
        let bits = self.bits + extra_bits;
        for range in &mut ret {
            range.bits = bits;
        }
        ret
    }
}

impl FromStr for Ipv4Range {
    type Err = IpRangeParseError;

    fn from_str(s: &str) -> Result<Ipv4Range, IpRangeParseError> {
        let mut split = s.split('/');
        let addr = unwrap!(split.next());
        let bits = match split.next() {
            Some(bits) => bits,
            None => return Err(IpRangeParseError::MissingDelimiter),
        };
        match split.next() {
            Some(..) => return Err(IpRangeParseError::ExtraDelimiter),
            None => (),
        };
        let addr = match Ipv4Addr::from_str(addr) {
            Ok(addr) => addr,
            Err(e) => return Err(IpRangeParseError::ParseAddr(e)),
        };
        let bits = match u8::from_str(bits) {
            Ok(bits) => bits,
            Err(e) => return Err(IpRangeParseError::ParseNetmaskPrefixLength(e)),
        };
        Ok(Ipv4Range::new(addr, bits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod ipv4range {
        use super::*;

        mod contains {
            use super::*;

            #[test]
            fn it_checks_if_address_is_within_the_range() {
                let addrs = Ipv4Range::new(ipv4!("1.2.3.0"), 24);

                assert!(addrs.contains(ipv4!("1.2.3.5")));
                assert!(addrs.contains(ipv4!("1.2.3.255")));
                assert!(!addrs.contains(ipv4!("1.2.4.5")));
            }

            #[test]
            fn when_range_is_global_it_returns_true_for_all_addresses() {
                let addrs = Ipv4Range::global();

                assert!(addrs.contains(ipv4!("1.2.3.5")));
                assert!(addrs.contains(ipv4!("192.168.1.255")));
                assert!(addrs.contains(ipv4!("255.255.255.255")));
            }
        }

        mod is_brodcast {
            use super::*;

            #[test]
            fn it_returns_false_if_address_is_not_even_in_the_range() {
                let addrs = Ipv4Range::new(ipv4!("1.2.3.0"), 24);

                assert!(!addrs.is_broadcast(ipv4!("1.2.4.5")));
            }

            #[test]
            fn it_returns_false_if_address_is_in_the_range_but_not_a_broadcast() {
                let addrs = Ipv4Range::new(ipv4!("1.2.3.0"), 24);
                assert!(!addrs.is_broadcast(ipv4!("1.2.3.4")));

                let addrs = Ipv4Range::new(ipv4!("1.2.0.0"), 16);
                assert!(!addrs.is_broadcast(ipv4!("1.2.3.255")));
            }

            #[test]
            fn it_returns_true_if_address_host_bits_are_all_1() {
                let addrs = Ipv4Range::new(ipv4!("1.2.3.0"), 24);
                assert!(addrs.is_broadcast(ipv4!("1.2.3.255")));

                let addrs = Ipv4Range::new(ipv4!("1.2.0.0"), 16);
                assert!(addrs.is_broadcast(ipv4!("1.2.255.255")));
            }

            #[test]
            fn it_returns_true_if_address_is_255_255_255_255() {
                let addrs = Ipv4Range::new(ipv4!("1.2.3.0"), 24);

                assert!(addrs.is_broadcast(ipv4!("255.255.255.255")));
            }
        }
    }
}
