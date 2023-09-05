use crate::priv_prelude::*;

/// An IPv4 address range, for example 192.168.0.0/16.
#[derive(Clone, Copy)]
pub struct Ipv4Network {
    base_addr: Ipv4Addr,
    subnet_mask_bits: u8,
}

/// An IPv6 address range, for example fc00::/7.
#[derive(Clone, Copy)]
pub struct Ipv6Network {
    base_addr: Ipv6Addr,
    subnet_mask_bits: u8,
}

impl Ipv4Network {
    /// Creates a network range containing `base_addr` and all addresses that share the same
    /// `subnet_mask_bits` initial bits.
    ///
    /// # Example
    ///
    /// `Ipv4Network::new(Ipv4Addr::from([192, 168, 0, 0]), 16)` is the network
    /// 192.168.0.0/16.
    pub fn new(base_addr: Ipv4Addr, subnet_mask_bits: u8) -> Ipv4Network {
        assert!(subnet_mask_bits <= 32);
        let mask = if subnet_mask_bits == 32 { !0u32 } else { !(!0 >> subnet_mask_bits) };
        let base_addr = Ipv4Addr::from(u32::from(base_addr) & mask);
        Ipv4Network { base_addr, subnet_mask_bits }
    }

    /// Returns the global network containing the entire address range.
    pub fn global() -> Ipv4Network {
        Ipv4Network::new(Ipv4Addr::UNSPECIFIED, 0)
    }

    /// Checks whether this range contains the given IPv4 address.
    pub fn contains(self, addr: Ipv4Addr) -> bool {
        let mask = if self.subnet_mask_bits == 32 { !0u32 } else { !(!0 >> self.subnet_mask_bits) };
        let addr_bits = u32::from(addr) & mask;
        let base_addr_bits = u32::from(self.base_addr);
        addr_bits == base_addr_bits
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
    /// address does not belong to a reserved range then this method will return the full range
    /// 0.0.0.0/0.
    pub fn infer_from_addr(addr: Ipv4Addr) -> Ipv4Network {
        let reserved_address_blocks = [
            Ipv4Network::new(ipv4!("0.0.0.0"), 8),
            Ipv4Network::new(ipv4!("0.0.0.0"), 32),
            Ipv4Network::new(ipv4!("10.0.0.0"), 8),
            Ipv4Network::new(ipv4!("100.64.0.0"), 10),
            Ipv4Network::new(ipv4!("127.0.0.0"), 8),
            Ipv4Network::new(ipv4!("169.254.0.0"), 16),
            Ipv4Network::new(ipv4!("172.16.0.0"), 12),
            Ipv4Network::new(ipv4!("192.0.0.0"), 24),
            Ipv4Network::new(ipv4!("192.0.0.0"), 29),
            Ipv4Network::new(ipv4!("192.0.0.8"), 32),
            Ipv4Network::new(ipv4!("192.0.0.9"), 32),
            Ipv4Network::new(ipv4!("192.0.0.10"), 32),
            Ipv4Network::new(ipv4!("192.0.0.170"), 32),
            Ipv4Network::new(ipv4!("192.0.2.0"), 24),
            Ipv4Network::new(ipv4!("192.31.196.0"), 24),
            Ipv4Network::new(ipv4!("192.52.193.0"), 24),
            Ipv4Network::new(ipv4!("192.88.99.0"), 24),
            Ipv4Network::new(ipv4!("192.168.0.0"), 16),
            Ipv4Network::new(ipv4!("192.175.48.0"), 24),
            Ipv4Network::new(ipv4!("198.18.0.0"), 15),
            Ipv4Network::new(ipv4!("198.51.100.0"), 24),
            Ipv4Network::new(ipv4!("203.0.113.0"), 24),
            Ipv4Network::new(ipv4!("240.0.0.0"), 4),
            Ipv4Network::new(ipv4!("255.255.255.255"), 32),
        ];
        reserved_address_blocks
        .into_iter()
        .filter(|network| network.contains(addr))
        .max_by_key(|network| network.subnet_mask_bits)
        .unwrap_or_else(|| Ipv4Network::new(Ipv4Addr::UNSPECIFIED, 0))
    }
}

impl Ipv6Network {
    /// Creates a network range containing `base_addr` and all addresses that share the same
    /// `subnet_mask_bits` initial bits.
    ///
    /// # Example
    ///
    /// `Ipv6Network::new(Ipv6Addr::from([0xfc00u16, 0, 0, 0, 0, 0, 0, 0]), 7)` is the network
    /// fc00::/7.
    pub fn new(base_addr: Ipv6Addr, subnet_mask_bits: u8) -> Ipv6Network {
        assert!(subnet_mask_bits <= 128);
        let mask = if subnet_mask_bits == 128 { !0u128 } else { !(!0 >> subnet_mask_bits) };
        let base_addr = Ipv6Addr::from(u128::from(base_addr) & mask);
        Ipv6Network { base_addr, subnet_mask_bits }
    }

    /// Returns the global network containing the entire address range.
    pub fn global() -> Ipv6Network {
        Ipv6Network::new(Ipv6Addr::UNSPECIFIED, 0)
    }

    /// Checks whether this range contains the given IPv6 address.
    pub fn contains(self, addr: Ipv6Addr) -> bool {
        let mask = if self.subnet_mask_bits == 128 { !0u128 } else { !(!0 >> self.subnet_mask_bits) };
        let addr_bits = u128::from(addr) & mask;
        let base_addr_bits = u128::from(self.base_addr);
        addr_bits == base_addr_bits
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
        let reserved_address_blocks = [
            Ipv6Network::new(ipv6!("::"), 128),
            Ipv6Network::new(ipv6!("::1"), 128),
            Ipv6Network::new(ipv6!("::ffff:0:0"), 96),
            Ipv6Network::new(ipv6!("::ffff:0:0:0"), 96),
            Ipv6Network::new(ipv6!("64:ff9b::"), 96),
            Ipv6Network::new(ipv6!("64:ff9b:1::"), 48),
            Ipv6Network::new(ipv6!("100::"), 64),
            Ipv6Network::new(ipv6!("2001:0000::"), 32),
            Ipv6Network::new(ipv6!("2001:20::"), 28),
            Ipv6Network::new(ipv6!("2001:db8::"), 32),
            Ipv6Network::new(ipv6!("2002::"), 16),
            Ipv6Network::new(ipv6!("fc00::"), 7),
            Ipv6Network::new(ipv6!("fe80::"), 64),
            Ipv6Network::new(ipv6!("ff00::"), 8),
        ];
        reserved_address_blocks
        .into_iter()
        .filter(|network| network.contains(addr))
        .max_by_key(|network| network.subnet_mask_bits)
        .unwrap_or_else(|| Ipv6Network::new(Ipv6Addr::UNSPECIFIED, 0))
    }
}

impl fmt::Debug for Ipv4Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Ipv4Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.base_addr, self.subnet_mask_bits)
    }
}

impl fmt::Debug for Ipv6Network {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
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
    next_addr: Ipv4Addr,
}

impl Iterator for Ipv4NetworkIter {
    type Item = Ipv4Addr;

    fn next(&mut self) -> Option<Ipv4Addr> {
        if !self.network.contains(self.next_addr) {
            return None;
        }
        let ret = self.next_addr;
        self.next_addr = Ipv4Addr::from(u32::from(self.next_addr).checked_add(1)?);
        Some(ret)
    }
}

/// An iterator over the IPv6 addresses in an [`Ipv6Network`](crate::Ipv6Network).
pub struct Ipv6NetworkIter {
    network: Ipv6Network,
    next_addr: Ipv6Addr,
}

impl Iterator for Ipv6NetworkIter {
    type Item = Ipv6Addr;

    fn next(&mut self) -> Option<Ipv6Addr> {
        if !self.network.contains(self.next_addr) {
            return None;
        }
        let ret = self.next_addr;
        self.next_addr = Ipv6Addr::from(u128::from(self.next_addr).checked_add(1)?);
        Some(ret)
    }
}

impl IntoIterator for Ipv4Network {
    type Item = Ipv4Addr;
    type IntoIter = Ipv4NetworkIter;

    fn into_iter(self) -> Ipv4NetworkIter {
        Ipv4NetworkIter {
            network: self,
            next_addr: self.base_addr(),
        }
    }
}

impl IntoIterator for Ipv6Network {
    type Item = Ipv6Addr;
    type IntoIter = Ipv6NetworkIter;

    fn into_iter(self) -> Ipv6NetworkIter {
        Ipv6NetworkIter {
            network: self,
            next_addr: self.base_addr(),
        }
    }
}

impl std::iter::FusedIterator for Ipv4NetworkIter {}
impl std::iter::FusedIterator for Ipv6NetworkIter {}

