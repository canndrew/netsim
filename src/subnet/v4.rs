use priv_prelude::*;
use super::*;
use rand;

/// An Ipv4 subnet
#[derive(Clone, Copy)]
pub struct SubnetV4 {
    addr: Ipv4Addr,
    bits: u8,
}

impl fmt::Debug for SubnetV4 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.addr, self.bits)
    }
}

impl SubnetV4 {
    /// Create a subnet with the given base IP address and number of netmask bits.
    ///
    /// # Example
    ///
    /// Create the subnet 192.168.0.0/24 with `SubnetV4::new(ipv4!("192.168.0.0"), 24)`
    pub fn new(addr: Ipv4Addr, bits: u8) -> SubnetV4 {
        let mask = !((!0u32).checked_shr(u32::from(bits)).unwrap_or(0));
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
        Ipv4Addr::from(!((!0u32).checked_shr(u32::from(self.bits)).unwrap_or(0)))
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

    /// Check whether this subnet contains the given IP address
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        let base_addr = u32::from(self.addr);
        let test_addr = u32::from(ip);
        (base_addr ^ test_addr).leading_zeros() >= u32::from(self.bits)
    }

    /// Split a subnet into `num` sub-subnets
    ///
    /// # Panics
    ///
    /// If the subnet is too small to be split up that much.
    pub fn split(self, num: u32) -> Vec<SubnetV4> {
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
            ret.push(SubnetV4 {
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
        for subnet in &mut ret {
            subnet.bits = bits;
        }
        ret
    }
}

impl FromStr for SubnetV4 {
    type Err = SubnetParseError;

    fn from_str(s: &str) -> Result<SubnetV4, SubnetParseError> {
        let mut split = s.split('/');
        let addr = unwrap!(split.next());
        let bits = match split.next() {
            Some(bits) => bits,
            None => return Err(SubnetParseError::MissingDelimiter),
        };
        match split.next() {
            Some(..) => return Err(SubnetParseError::ExtraDelimiter),
            None => (),
        };
        let addr = match Ipv4Addr::from_str(addr) {
            Ok(addr) => addr,
            Err(e) => return Err(SubnetParseError::ParseAddr(e)),
        };
        let bits = match u8::from_str(bits) {
            Ok(bits) => bits,
            Err(e) => return Err(SubnetParseError::ParseBits(e)),
        };
        Ok(SubnetV4::new(addr, bits))
    }
}

