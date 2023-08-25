use crate::priv_prelude::*;

#[derive(Clone, Copy)]
pub struct Ipv4Network {
    base_addr: Ipv4Addr,
    subnet_mask_bits: u8,
}

impl Ipv4Network {
    pub fn new(base_addr: Ipv4Addr, subnet_mask_bits: u8) -> Ipv4Network {
        assert!(subnet_mask_bits <= 32);
        let mask = if subnet_mask_bits == 32 { !0u32 } else { !(!0 >> subnet_mask_bits) };
        let base_addr = Ipv4Addr::from(u32::from(base_addr) & mask);
        Ipv4Network { base_addr, subnet_mask_bits }
    }

    pub fn contains(self, addr: Ipv4Addr) -> bool {
        let mask = if self.subnet_mask_bits == 32 { !0u32 } else { !(!0 >> self.subnet_mask_bits) };
        let addr_bits = u32::from(addr) & mask;
        let base_addr_bits = u32::from(self.base_addr);
        addr_bits == base_addr_bits
    }

    pub fn base_addr(self) -> Ipv4Addr {
        self.base_addr
    }

    pub fn subnet_mask_bits(self) -> u8 {
        self.subnet_mask_bits
    }

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
        .unwrap_or_else(|| Ipv4Network::new(Ipv4Addr::from([0, 0, 0, 0]), 0))
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
