use crate::priv_prelude::*;

/// A list of special-purpose IPv4 address ranges. Any address for which `Ipv4Addr::is_global` is
/// false is guaranteed to be contained in one of these networks.
pub static RESERVED_IPV4_NETWORKS: &[Ipv4Network] = &[
    ipv4_network!("0.0.0.0/8"),
    ipv4_network!("0.0.0.0/32"),
    ipv4_network!("10.0.0.0/8"),
    ipv4_network!("100.64.0.0/10"),
    ipv4_network!("127.0.0.0/8"),
    ipv4_network!("169.254.0.0/16"),
    ipv4_network!("172.16.0.0/12"),
    ipv4_network!("192.0.0.0/24"),
    ipv4_network!("192.0.0.0/29"),
    ipv4_network!("192.0.0.8/32"),
    ipv4_network!("192.0.0.9/32"),
    ipv4_network!("192.0.0.10/32"),
    ipv4_network!("192.0.0.170/32"),
    ipv4_network!("192.0.2.0/24"),
    ipv4_network!("192.31.196.0/24"),
    ipv4_network!("192.52.193.0/24"),
    ipv4_network!("192.88.99.0/24"),
    ipv4_network!("192.168.0.0/16"),
    ipv4_network!("192.175.48.0/24"),
    ipv4_network!("198.18.0.0/15"),
    ipv4_network!("198.51.100.0/24"),
    ipv4_network!("203.0.113.0/24"),
    ipv4_network!("224.0.0.0/4"),
    ipv4_network!("233.252.0.0/24"),
    ipv4_network!("240.0.0.0/4"),
    ipv4_network!("255.255.255.255/32"),
];

/// A list of special-purpose IPv6 address ranges. Any address for which `Ipv6Addr::is_global` is
/// false is guaranteed to be contained in one of these networks.
pub static RESERVED_IPV6_NETWORKS: &[Ipv6Network] = &[
    ipv6_network!("::/128"),
    ipv6_network!("::1/128"),
    ipv6_network!("::ffff:0:0/96"),
    ipv6_network!("::ffff:0:0:0/96"),
    ipv6_network!("64:ff9b::/96"),
    ipv6_network!("64:ff9b:1::/48"),
    ipv6_network!("100::/64"),
    ipv6_network!("2001:0000::/32"),
    ipv6_network!("2001:20::/28"),
    ipv6_network!("2001:db8::/32"),
    ipv6_network!("2002::/16"),
    ipv6_network!("fc00::/7"),
    ipv6_network!("fe80::/64"),
    ipv6_network!("ff00::/8"),
];

/// An IPv4 address range, for example 192.168.0.0/16.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Network {
    base_addr: Ipv4Addr,
    subnet_mask_bits: u8,
}

/// An IPv6 address range, for example fc00::/7.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Ipv6Network {
    base_addr: Ipv6Addr,
    subnet_mask_bits: u8,
}

impl Ipv4Network {
    /// The global network containing the entire IPv4 address range.
    pub const GLOBAL: Ipv4Network = ipv4_network!("0.0.0.0/0");

    /// Creates a network range containing `base_addr` and all addresses that share the same
    /// `subnet_mask_bits` most-significant bits.
    ///
    /// # Example
    ///
    /// `Ipv4Network::new(Ipv4Addr::from([192, 168, 0, 0]), 16)` is the network
    /// 192.168.0.0/16.
    pub const fn new(base_addr: Ipv4Addr, subnet_mask_bits: u8) -> Ipv4Network {
        assert!(subnet_mask_bits <= 32);
        let mask = if subnet_mask_bits == 32 { !0u32 } else { !(!0 >> subnet_mask_bits) };

        // NOTE: Can't use simple `From` conversions here because we need to be const.

        let [b0, b1, b2, b3] = base_addr.octets();
        let base_addr_bits = {
            ((b0 as u32) << 24) |
            ((b1 as u32) << 16) |
            ((b2 as u32) << 8) |
            (b3 as u32)
        };
        let base_addr_bits = base_addr_bits & mask;
        let b0 = (base_addr_bits >> 24) as u8;
        let b1 = ((base_addr_bits >> 16) & 0xff) as u8;
        let b2 = ((base_addr_bits >> 8) & 0xff) as u8;
        let b3 = (base_addr_bits & 0xff) as u8;
        let base_addr = Ipv4Addr::new(b0, b1, b2, b3);
        Ipv4Network { base_addr, subnet_mask_bits }
    }

    /// Checks whether this range contains the given IPv4 address.
    pub fn contains(self, addr: Ipv4Addr) -> bool {
        let mask = if self.subnet_mask_bits == 32 { !0u32 } else { !(!0 >> self.subnet_mask_bits) };
        let addr_bits = u32::from(addr) & mask;
        let base_addr_bits = u32::from(self.base_addr);
        addr_bits == base_addr_bits
    }

    /// Checks whether this range contains the given range.
    pub fn contains_network(self, other: Ipv4Network) -> bool {
        (self.subnet_mask_bits <= other.subnet_mask_bits) &&
        (self.base_addr == Ipv4Addr::from(u32::from(other.base_addr) & !(!0 >> self.subnet_mask_bits)))
    }

    /// The base address of the network range. Not necessarily the same value passed to
    /// `Ipv4Network::new` since the address will be masked to zero all bits after the first
    /// `subnet_mask_bits`.
    pub fn base_addr(self) -> Ipv4Addr {
        self.base_addr
    }

    /// The length of the subnet mask.
    pub fn subnet_mask_bits(self) -> u8 {
        self.subnet_mask_bits
    }

    /// Guesses an `Ipv4Network` from an `Ipv4Addr` by checking which, if any, of the [reserved IP
    /// address ranges](https://en.wikipedia.org/wiki/Reserved_IP_addresses) it belongs to. If the
    /// address does not belong to a reserved range then this method will return the global range
    /// 0.0.0.0/0.
    pub fn infer_from_addr(addr: Ipv4Addr) -> Ipv4Network {
        RESERVED_IPV4_NETWORKS
        .iter()
        .copied()
        .filter(|network| network.contains(addr))
        .max_by_key(|network| network.subnet_mask_bits)
        .unwrap_or_else(|| Ipv4Network::GLOBAL)
    }

    /// Generate a random address in this range and not in any reserved IPv4 range strictly
    /// contained within this range.
    pub fn random_addr(&self, rng: &mut impl rand::Rng) -> Ipv4Addr {
        let mask = !0u32 >> self.subnet_mask_bits;
        'start: loop {
            let addr = Ipv4Addr::from((rng.gen::<u32>() & mask) | u32::from(self.base_addr));
            for network in RESERVED_IPV4_NETWORKS {
                if self.contains_network(*network) && *self != *network && network.contains(addr) {
                    continue 'start;
                }
            }
            break addr;
        }
    }
}

impl Ipv6Network {
    /// The global network containing the entire IPv6 address range.
    pub const GLOBAL: Ipv6Network = ipv6_network!("::/0");

    /// Creates a network range containing `base_addr` and all addresses that share the same
    /// `subnet_mask_bits` initial bits.
    ///
    /// # Example
    ///
    /// `Ipv6Network::new(Ipv6Addr::from([0xfc00u16, 0, 0, 0, 0, 0, 0, 0]), 7)` is the network
    /// fc00::/7.
    pub const fn new(base_addr: Ipv6Addr, subnet_mask_bits: u8) -> Ipv6Network {
        assert!(subnet_mask_bits <= 128);
        let mask = if subnet_mask_bits == 128 { !0u128 } else { !(!0 >> subnet_mask_bits) };

        // NOTE: Can't use simple `From` conversions here because we need to be const.

        let [s0, s1, s2, s3, s4, s5, s6, s7] = base_addr.segments();
        let base_addr_bits = {
            ((s0 as u128) << 112) |
            ((s1 as u128) << 96) |
            ((s2 as u128) << 80) |
            ((s3 as u128) << 64) |
            ((s4 as u128) << 48) |
            ((s5 as u128) << 32) |
            ((s6 as u128) << 16) |
            (s7 as u128)
        };
        let base_addr_bits = base_addr_bits & mask;
        let s0 = ((base_addr_bits >> 112) & 0xffff) as u16;
        let s1 = ((base_addr_bits >> 96) & 0xffff) as u16;
        let s2 = ((base_addr_bits >> 80) & 0xffff) as u16;
        let s3 = ((base_addr_bits >> 64) & 0xffff) as u16;
        let s4 = ((base_addr_bits >> 48) & 0xffff) as u16;
        let s5 = ((base_addr_bits >> 32) & 0xffff) as u16;
        let s6 = ((base_addr_bits >> 16) & 0xffff) as u16;
        let s7 = (base_addr_bits & 0xffff) as u16;
        let base_addr = Ipv6Addr::new(s0, s1, s2, s3, s4, s5, s6, s7);
        Ipv6Network { base_addr, subnet_mask_bits }
    }

    /// Checks whether this range contains the given IPv6 address.
    pub fn contains(self, addr: Ipv6Addr) -> bool {
        let mask = if self.subnet_mask_bits == 128 { !0u128 } else { !(!0 >> self.subnet_mask_bits) };
        let addr_bits = u128::from(addr) & mask;
        let base_addr_bits = u128::from(self.base_addr);
        addr_bits == base_addr_bits
    }

    /// Checks whether this range contains the given range.
    pub fn contains_network(self, other: Ipv6Network) -> bool {
        (self.subnet_mask_bits <= other.subnet_mask_bits) &&
        (self.base_addr == Ipv6Addr::from(u128::from(other.base_addr) & !(!0 >> self.subnet_mask_bits)))
    }

    /// The base address of the network range. Not necessarily the same value passed to
    /// `Ipv6Network::new` since the address will be masked to zero all bits after the first
    /// `subnet_mask_bits`.
    pub fn base_addr(self) -> Ipv6Addr {
        self.base_addr
    }

    /// The length of the subnet mask.
    pub fn subnet_mask_bits(self) -> u8 {
        self.subnet_mask_bits
    }

    /// Guesses an `Ipv6Network` from an `Ipv6Addr` by checking which, if any, of the [reserved IP
    /// address ranges](https://en.wikipedia.org/wiki/Reserved_IP_addresses) it belongs to. If the
    /// address does not belong to a reserved range then this method will return the full range
    /// ::/0.
    pub fn infer_from_addr(addr: Ipv6Addr) -> Ipv6Network {
        RESERVED_IPV6_NETWORKS
        .iter()
        .copied()
        .filter(|network| network.contains(addr))
        .max_by_key(|network| network.subnet_mask_bits)
        .unwrap_or_else(|| Ipv6Network::GLOBAL)
    }

    /// Generate a random address in this range and not in any reserved IPv6 range strictly
    /// contained within this range.
    pub fn random_addr(&self) -> Ipv6Addr {
        let mask = !0u128 >> self.subnet_mask_bits;
        'start: loop {
            let addr = Ipv6Addr::from((rand::random::<u128>() & mask) | u128::from(self.base_addr));
            for network in RESERVED_IPV6_NETWORKS {
                if self.contains_network(*network) && *self != *network && network.contains(addr) {
                    continue 'start;
                }
            }
            break addr;
        }
    }
}

impl fmt::Debug for Ipv4Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ipv4_network!({:?})", self.to_string())
    }
}

impl fmt::Display for Ipv4Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.base_addr, self.subnet_mask_bits)
    }
}

impl fmt::Debug for Ipv6Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ipv6_network!({:?})", self.to_string())
    }
}

impl fmt::Display for Ipv6Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.base_addr, self.subnet_mask_bits)
    }
}

impl str::FromStr for Ipv4Network {
    type Err = NetworkParseError;

    fn from_str(s: &str) -> Result<Ipv4Network, NetworkParseError> {
        let res = match s.split_once('/') {
            None => Err(None),
            Some((addr, subnet_mask_bits)) => {
                match Ipv4Addr::from_str(addr) {
                    Err(err) => Err(Some(err)),
                    Ok(addr) => {
                        match u8::from_str(subnet_mask_bits) {
                            Err(_err) => Err(None),
                            Ok(subnet_mask_bits) => {
                                if subnet_mask_bits <= 32 {
                                    Ok(Ipv4Network::new(addr, subnet_mask_bits))
                                } else {
                                    Err(None)
                                }
                            },
                        }
                    },
                }
            },
        };
        match res {
            Ok(ipv4_network) => Ok(ipv4_network),
            Err(addr_parse_error_opt) => {
                let err = NetworkParseError {
                    s: s.to_owned(),
                    addr_parse_error_opt,
                };
                Err(err)
            },
        }
    }
}

impl str::FromStr for Ipv6Network {
    type Err = NetworkParseError;

    fn from_str(s: &str) -> Result<Ipv6Network, NetworkParseError> {
        let res = match s.split_once('/') {
            None => Err(None),
            Some((addr, subnet_mask_bits)) => {
                match Ipv6Addr::from_str(addr) {
                    Err(err) => Err(Some(err)),
                    Ok(addr) => {
                        match u8::from_str(subnet_mask_bits) {
                            Err(_err) => Err(None),
                            Ok(subnet_mask_bits) => {
                                if subnet_mask_bits <= 128 {
                                    Ok(Ipv6Network::new(addr, subnet_mask_bits))
                                } else {
                                    Err(None)
                                }
                            },
                        }
                    },
                }
            },
        };
        match res {
            Ok(ipv6_network) => Ok(ipv6_network),
            Err(addr_parse_error_opt) => {
                let err = NetworkParseError {
                    s: s.to_owned(),
                    addr_parse_error_opt,
                };
                Err(err)
            },
        }
    }
}

/// Error that can be returned when parsing an [`Ipv4Network`](crate::Ipv4Network) or
/// [`Ipv6Network`](crate::Ipv6Network).
#[derive(Debug)]
pub struct NetworkParseError {
    s: String,
    addr_parse_error_opt: Option<std::net::AddrParseError>,
}

impl fmt::Display for NetworkParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} is not valid IP network syntax", self.s)
    }
}

impl std::error::Error for NetworkParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        let err = self.addr_parse_error_opt.as_ref()?;
        Some(err)
    }
}

/// An iterator over the IPv4 addresses in an [`Ipv4Network`](crate::Ipv4Network).
pub struct Ipv4NetworkIter {
    network: Ipv4Network,
    next_addr_opt: Option<Ipv4Addr>,
}

impl Iterator for Ipv4NetworkIter {
    type Item = Ipv4Addr;

    fn next(&mut self) -> Option<Ipv4Addr> {
        let next_addr = self.next_addr_opt.take()?;
        if !self.network.contains(next_addr) {
            return None;
        }
        self.next_addr_opt = u32::from(next_addr).checked_add(1).map(Ipv4Addr::from);
        Some(next_addr)
    }
}

/// An iterator over the IPv6 addresses in an [`Ipv6Network`](crate::Ipv6Network).
pub struct Ipv6NetworkIter {
    network: Ipv6Network,
    next_addr_opt: Option<Ipv6Addr>,
}

impl Iterator for Ipv6NetworkIter {
    type Item = Ipv6Addr;

    fn next(&mut self) -> Option<Ipv6Addr> {
        let next_addr = self.next_addr_opt.take()?;
        if !self.network.contains(next_addr) {
            return None;
        }
        self.next_addr_opt = u128::from(next_addr).checked_add(1).map(Ipv6Addr::from);
        Some(next_addr)
    }
}

impl IntoIterator for Ipv4Network {
    type Item = Ipv4Addr;
    type IntoIter = Ipv4NetworkIter;

    fn into_iter(self) -> Ipv4NetworkIter {
        Ipv4NetworkIter {
            network: self,
            next_addr_opt: Some(self.base_addr()),
        }
    }
}

impl IntoIterator for Ipv6Network {
    type Item = Ipv6Addr;
    type IntoIter = Ipv6NetworkIter;

    fn into_iter(self) -> Ipv6NetworkIter {
        Ipv6NetworkIter {
            network: self,
            next_addr_opt: Some(self.base_addr()),
        }
    }
}

impl std::iter::FusedIterator for Ipv4NetworkIter {}
impl std::iter::FusedIterator for Ipv6NetworkIter {}

