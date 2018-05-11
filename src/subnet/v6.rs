use priv_prelude::*;

#[derive(Clone, Copy)]
/// An IPv6 subnet
pub struct SubnetV6 {
    addr: Ipv6Addr,
    bits: u8,
}

impl fmt::Debug for SubnetV6 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.addr, self.bits)
    }
}

impl SubnetV6 {
    /// Create a subnet with the given base IP address and number of netmask bits.
    ///
    /// # Example
    ///
    /// Create the subnet 2000::/12 with `SubnetV6::new(ipv6!("2000::"), 12)`
    pub fn new(addr: Ipv6Addr, bits: u8) -> SubnetV6 {
        let mask = !((!0u128).checked_shr(u32::from(bits)).unwrap_or(0));
        SubnetV6 {
            addr: Ipv6Addr::from(u128::from(addr) & mask),
            bits: bits,
        }
    }

    /// Return the global subnet, ::/0
    pub fn global() -> SubnetV6 {
        SubnetV6 {
            addr: ipv6!("::"),
            bits: 0,
        }
    }

    /// Get the netmask as an IP address
    pub fn netmask(&self) -> Ipv6Addr {
        Ipv6Addr::from(!((!0u128).checked_shr(u32::from(self.bits)).unwrap_or(0)))
    }

    /// Get the number of netmask bits
    pub fn netmask_bits(&self) -> u8 {
        self.bits
    }

    /// Get the base address of the subnet, ie. the lowest IP address which is part of the subnet.
    pub fn base_addr(&self) -> Ipv6Addr {
        self.addr
    }

    /// Check whether this subnet contains the given IP address
    pub fn contains(&self, ip: Ipv6Addr) -> bool {
        let base_addr = u128::from(self.addr);
        let test_addr = u128::from(ip);
        (base_addr ^ test_addr).leading_zeros() >= u32::from(self.bits)
    }
}

