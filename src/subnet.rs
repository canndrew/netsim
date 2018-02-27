use priv_prelude::*;
use rand;
use std::net::AddrParseError;
use std::num::ParseIntError;

/// An Ipv4 subnet
#[derive(Debug, Clone, Copy)]
pub struct SubnetV4 {
    addr: Ipv4Addr,
    bits: u8,
}

impl SubnetV4 {
    /// Create a subnet with the given base IP address and number of netmask bits.
    ///
    /// # Example
    ///
    /// Create the subnet 192.168.0.0/24 with `SubnetV4::new(ipv4!("192.168.0.0"), 24)`
    pub fn new(addr: Ipv4Addr, bits: u8) -> SubnetV4 {
        let mask = !(0xffff_ffffu32.checked_shr(u32::from(bits)).unwrap_or(0));
        SubnetV4 {
            addr: Ipv4Addr::from(u32::from(addr) & mask),
            bits: bits,
        }
    }

    /// Return the global subnet, eg. 0.0.0.0/0
    pub fn global() -> SubnetV4 {
        SubnetV4 {
            addr: ipv4!("0.0.0.0"),
            bits: 0,
        }
    }

    /// Returns the local network subnet 10.0.0.0/8
    pub fn local_10() -> SubnetV4 {
        SubnetV4 {
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
    pub fn local_172(block: u8) -> SubnetV4 {
        assert!(block < 16);
        SubnetV4 {
            addr: Ipv4Addr::new(172, 16 | block, 0, 0),
            bits: 16,
        }
    }

    /// Returns the local subnet 192.168.x.0/24 where x is given by `block`.
    pub fn local_192(block: u8) -> SubnetV4 {
        SubnetV4 {
            addr: Ipv4Addr::new(192, 168, block, 0),
            bits: 24,
        }
    }

    /// Returns a random local network from one of the ranges 10.0.0.0, 172.16.0.0 or 192.168.0.0
    pub fn random_local() -> SubnetV4 {
        #[derive(Rand)]
        enum Subnet {
            S10,
            S172(u8),
            S192(u8),
        };

        match rand::random() {
            Subnet::S10 => SubnetV4::local_10(),
            Subnet::S172(x) => SubnetV4::local_172(x & 0x0f),
            Subnet::S192(x) => SubnetV4::local_192(x),
        }
    }

    /// Get the netmask as an IP address
    pub fn netmask(&self) -> Ipv4Addr {
        Ipv4Addr::from(!(0xffff_ffffu32.checked_shr(u32::from(self.bits)).unwrap_or(0)))
    }

    /// Get the number of netmask bits
    pub fn netmask_bits(&self) -> u8 {
        self.bits
    }

    /// Get the base address of the subnet, ie. the lowest IP address which is part of the subnet.
    pub fn base_addr(&self) -> Ipv4Addr {
        self.addr
    }

    /// Get a default IP address for the subnet's gateway. This is one higher than the base address
    /// of the subnet. eg. for 10.0.0.0/8, the default address for the gateway will be 10.0.0.1
    pub fn gateway_ip(&self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.addr) | 1)
    }

    /// Get a random IP address from the subnet which is not the base address or the default
    /// for the gateway address.
    pub fn random_client_addr(&self) -> Ipv4Addr {
        let mask = 0xffff_ffff >> self.bits;
        assert!(mask > 1);

        let addr = loop {
            let x: u32 = rand::random();
            if x & mask > 1 {
                break x;
            }
        };
        let addr = u32::from(self.addr) | (addr & mask);
        Ipv4Addr::from(addr)
    }

    /// Check whether this subnet contains the given IP address
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        let base_addr = u32::from(self.addr);
        let test_addr = u32::from(ip);
        (base_addr ^ test_addr).leading_zeros() >= u32::from(self.bits)
    }
}

impl FromStr for SubnetV4 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<SubnetV4, ParseError> {
        let mut split = s.split('/');
        let addr = unwrap!(split.next());
        let bits = match split.next() {
            Some(bits) => bits,
            None => return Err(ParseError::MissingDelimiter),
        };
        match split.next() {
            Some(..) => return Err(ParseError::ExtraDelimiter),
            None => (),
        };
        let addr = match Ipv4Addr::from_str(addr) {
            Ok(addr) => addr,
            Err(e) => return Err(ParseError::ParseAddr(e)),
        };
        let bits = match u8::from_str(bits) {
            Ok(bits) => bits,
            Err(e) => return Err(ParseError::ParseBits(e)),
        };
        Ok(SubnetV4::new(addr, bits))
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum ParseError {
        MissingDelimiter {
            description("missing '/' delimiter")
        }
        ExtraDelimiter {
            description("more than one '/' delimiter")
        }
        ParseAddr(e: AddrParseError) {
            description("error parsing ipv4 address")
            display("error parsing ipv4 address: {}", e)
            cause(e)
        }
        ParseBits(e: ParseIntError) {
            description("error parsing subnet bit number")
            display("error parsing subnet bit number: {}", e)
            cause(e)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SubnetV6 {
    addr: Ipv6Addr,
    bits: u8,
}

