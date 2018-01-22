use priv_prelude::*;

/// A MAC address, the hardware address of an ethernet interface.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Rand)]
pub struct MacAddr {
    bytes: [u8; 6],
}

impl MacAddr {
    /// Create a mac address from a slice of 6 bytes.
    ///
    /// # Panic
    ///
    /// If the slice is not exactly 6 bytes.
    pub fn from_bytes(b: &[u8]) -> MacAddr {
        let mut bytes = [0u8; 6];
        bytes[..].clone_from_slice(b);
        MacAddr {
            bytes,
        }
    }

    /// Returns the mac address as an array of 6 bytes.
    pub fn as_bytes(&self) -> [u8; 6] {
        self.bytes
    }

    /// Returns the broadcast MAC address FF:FF:FF:FF:FF:FF
    pub fn broadcast() -> MacAddr {
        MacAddr {
            bytes: [0xff; 6],
        }
    }

    /// Check whether a frame with the MAC address should be received by an interface with address `iface`. Returns `true` if either `self` is the broadcast address or the addresses are equal
    pub fn matches(self, iface: MacAddr) -> bool {
        self == MacAddr::broadcast() || self == iface
    }
}

impl fmt::Debug for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                  self.bytes[0], self.bytes[1], self.bytes[2],
                  self.bytes[3], self.bytes[4], self.bytes[5])
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}


