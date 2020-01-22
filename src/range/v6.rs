use crate::priv_prelude::*;
use rand;

#[derive(Clone, Copy)]
/// A range of IPv6 addresses with a common prefix
pub struct Ipv6Range {
    addr: Ipv6Addr,
    bits: u8,
}

impl fmt::Debug for Ipv6Range {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.addr, self.bits)
    }
}

impl Ipv6Range {
    /// Create a range with the given base IP address and netmask prefix length.
    ///
    /// # Example
    ///
    /// Create the subnet 2000::/12 with `Ipv6Range::new(ipv6!("2000::"), 12)`
    pub fn new(addr: Ipv6Addr, bits: u8) -> Ipv6Range {
        let mask = !((!0u128).checked_shr(u32::from(bits)).unwrap_or(0));
        Ipv6Range {
            addr: Ipv6Addr::from(u128::from(addr) & mask),
            bits,
        }
    }

    /// Return the entire IPv6 range, ::/0
    pub fn global() -> Ipv6Range {
        Ipv6Range {
            addr: ipv6!("::"),
            bits: 0,
        }
    }

    /// Get the netmask as an IPv6 address
    pub fn netmask(&self) -> Ipv6Addr {
        Ipv6Addr::from(!((!0u128).checked_shr(u32::from(self.bits)).unwrap_or(0)))
    }

    /// Get the number of netmask prefix bits
    pub fn netmask_prefix_length(&self) -> u8 {
        self.bits
    }

    /// Get the base address of the range, ie. the lowest IP address which is part of the range.
    pub fn base_addr(&self) -> Ipv6Addr {
        self.addr
    }

    /// Check whether this range contains the given IP address
    pub fn contains(&self, ip: Ipv6Addr) -> bool {
        let base_addr = u128::from(self.addr);
        let test_addr = u128::from(ip);
        (base_addr ^ test_addr).leading_zeros() >= u32::from(self.bits)
    }

    /// Split a range into `num` sub-ranges
    ///
    /// # Panics
    ///
    /// If the range is too small to be split up that much.
    pub fn split(self, num: u32) -> Vec<Ipv6Range> {
        let mut ret = Vec::with_capacity(num as usize);
        let mut n = 0u128;
        let class = if self.bits == 0 {
            Ipv6AddrClass::Global
        } else {
            self.addr.class()
        };
        loop {
            let mut n_reversed = 0;
            for i in 0..128 {
                if n & (1 << i) != 0 {
                    n_reversed |= 0x8000_0000_0000_0000_0000_0000_0000_0000u128 >> i;
                }
            }
            let base_addr = u128::from(self.addr);
            let ip = base_addr | (n_reversed >> self.bits);
            let ip = Ipv6Addr::from(ip);
            if class != ip.class() {
                n += 1;
                continue;
            }
            ret.push(Ipv6Range { addr: ip, bits: 0 });
            if ret.len() == num as usize {
                break;
            }
            n += 1;
        }
        let extra_bits = (128 - n.leading_zeros()) as u8;
        let bits = self.bits + extra_bits;
        for range in &mut ret {
            range.bits = bits;
        }
        ret
    }

    /// Get a random IP address from the range which is not the base address or the default
    /// for the gateway address.
    pub fn random_client_addr(&self) -> Ipv6Addr {
        let mask = !0 >> self.bits;
        assert!(mask > 1);
        let class = if self.bits == 0 {
            Ipv6AddrClass::Global
        } else {
            self.addr.class()
        };

        loop {
            // `impl Rand for u128` doesn't exist yet
            let r0 = rand::random::<u64>();
            let r1 = rand::random::<u64>();
            let r = (u128::from(r1) << 64) | u128::from(r0);

            let x = r & mask;
            if x < 2 {
                continue;
            }
            let addr = Ipv6Addr::from(u128::from(self.addr) | x);
            if class != addr.class() {
                continue;
            }
            return addr;
        }
    }

    /// Get a default IP address for the range's next hop. This is one higher than the base address
    /// of the range. eg. for 2000::/x the default address for the next hop will be 2000::1
    pub fn next_hop_ip(&self) -> Ipv6Addr {
        Ipv6Addr::from(u128::from(self.addr) | 1)
    }
}
