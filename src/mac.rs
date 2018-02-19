//pub use priv_prelude::*;
use rand;

#[derive(Clone, Copy)]
pub struct MacAddr {
    bytes: [u8; 6],
}

impl MacAddr {
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

    pub fn random(&self) -> MacAddr {
        let mut b: [u8; 6] = rand::random();
        b[0] &= 0xfc;
        MacAddr {
            bytes: b,
        }
    }
}

