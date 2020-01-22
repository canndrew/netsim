use crate::priv_prelude::*;
use rand;

/// An ethernet hardware MAC address.
#[derive(Clone, Copy, PartialEq)]
pub struct MacAddr {
    bytes: [u8; 6],
}

impl fmt::Debug for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.bytes[0],
            self.bytes[1],
            self.bytes[2],
            self.bytes[3],
            self.bytes[4],
            self.bytes[5],
        )
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", *self)
    }
}

impl MacAddr {
    /// The broadcast ethernet address.
    pub const BROADCAST: MacAddr = MacAddr {
        bytes: [0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
    };

    /// Create a `MacAddr` from the given 6-byte buffer.
    pub fn from_bytes(bytes: &[u8]) -> MacAddr {
        let mut b = [0u8; 6];
        b[..].clone_from_slice(bytes);
        MacAddr { bytes: b }
    }

    /// Get the address as a slice of bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..]
    }

    /// Generate a random MAC address.
    pub fn random() -> MacAddr {
        let mut b: [u8; 6] = rand::random();
        b[0] &= 0xfc;
        MacAddr { bytes: b }
    }

    /// Checks weather this is the broadcast address.
    pub fn is_broadcast(&self) -> bool {
        *self == MacAddr::BROADCAST
    }
}
