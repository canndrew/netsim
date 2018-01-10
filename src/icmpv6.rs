use priv_prelude::*;

#[derive(Debug, Clone)]
pub struct Icmpv6Packet {
    data: Bytes,
}

impl Icmpv6Packet {
    pub fn from_bytes(data: Bytes) -> Icmpv6Packet {
        //assert_eq!(data.len(), 8);
        Icmpv6Packet {
            data,
        }
    }

    pub fn as_bytes(&self) -> &Bytes {
        &self.data
    }

    pub fn ty(&self) -> u8 {
        self.data[0]
    }

    pub fn code(&self) -> u8 {
        self.data[1]
    }
}

