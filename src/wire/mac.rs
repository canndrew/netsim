pub use priv_prelude::*;
use rand;

#[derive(Clone, Copy, PartialEq)]
pub struct MacAddr {
    bytes: [u8; 6],
}

impl fmt::Debug for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.bytes[0],
            self.bytes[1],
            self.bytes[2],
            self.bytes[3],
            self.bytes[4],
            self.bytes[5],
        )
    }
}

impl MacAddr {
    pub const BROADCAST: MacAddr = MacAddr {
        bytes: [0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
    };

    pub fn from_bytes(bytes: &[u8]) -> MacAddr {
        let mut b = [0u8; 6];
        b[..].clone_from_slice(bytes);
        MacAddr {
            bytes: b,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..]
    }

    pub fn random() -> MacAddr {
        let mut b: [u8; 6] = rand::random();
        b[0] &= 0xfc;
        MacAddr {
            bytes: b,
        }
    }

    pub fn is_broadcast(&self) -> bool {
        *self == MacAddr::BROADCAST
    }
}

